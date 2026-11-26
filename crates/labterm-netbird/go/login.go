package main

/*
#include <stdint.h>
*/
import "C"

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/netbirdio/netbird/client/iface/netstack"
	"github.com/netbirdio/netbird/client/system"
	mgm "github.com/netbirdio/netbird/shared/management/client"
	"golang.org/x/oauth2"
	"golang.zx2c4.com/wireguard/wgctrl/wgtypes"
)

// SSO sign-in: each person joins a network as their own peer by signing in
// at the identity provider the NetBird server is set up with (Authentik, …),
// so NetBird's users, groups and policies decide who gets in. The flow is
// the one NetBird's own apps use: authorization code with PKCE, the browser
// redirecting to a short-lived listener on localhost. The token only
// registers (or re-authorizes) this device's WireGuard key; later starts log
// in with that key alone until the server's login expiry asks again.

type loginOptions struct {
	ManagementURL string `json:"management_url"`
	// The device's peer key from an earlier start; empty registers a new
	// peer. Signing in again with the same key keeps one peer per phone.
	PrivateKey string `json:"private_key"`
	DeviceName string `json:"device_name"`
	// The identity provider account to sign in as (mke-adm rather than
	// mke); empty = whoever signs in.
	Account string `json:"account"`
}

type loginBegin struct {
	AuthURL string `json:"auth_url"`
	// How long each step took, for the log ("probe 120ms, ...").
	Timings string `json:"timings,omitempty"`
}

type loginResult struct {
	PrivateKey string `json:"private_key"`
	Email      string `json:"email,omitempty"`
	Timings    string `json:"timings,omitempty"`
}

// steps records how long each step of a sign-in took.
type steps struct {
	last time.Time
	out  []string
}

func newSteps() *steps { return &steps{last: time.Now()} }

func (s *steps) done(name string) {
	now := time.Now()
	s.out = append(s.out, fmt.Sprintf("%s %s", name, now.Sub(s.last).Round(time.Millisecond)))
	s.last = now
}

func (s *steps) String() string { return strings.Join(s.out, ", ") }

type pendingLogin struct {
	// Extra listener on [::1]: browsers may try that first for "localhost".
	listener6 net.Listener
	opts      loginOptions
	key       wgtypes.Key
	oauth     oauth2.Config
	useID     bool
	state     string
	verifier  string
	server    *http.Server
	codes     chan string
	errs      chan error
	// Step timings of beginLogin, for the log.
	timings string
	once    sync.Once
	// Set while tnb_login_finish waits; otherwise a cancel closes the
	// listener itself, so an abandoned sign-in never keeps its port.
	finishing atomic.Bool
}

// Sign-ins in progress by handle. A map rather than cgo.Handle so a late
// cancel for a finished sign-in is a no-op instead of a crash.
var (
	loginsMu  sync.Mutex
	logins    = map[uint64]*pendingLogin{}
	nextLogin uint64
)

func (p *pendingLogin) close() {
	p.once.Do(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		_ = p.server.Shutdown(ctx)
		if p.listener6 != nil {
			p.listener6.Close()
		}
	})
}

// mgmAddr turns a management URL into the host:port and TLS flag the gRPC
// client wants.
func mgmAddr(raw string) (string, bool, error) {
	u, err := url.Parse(raw)
	if err != nil {
		return "", false, err
	}
	tls := u.Scheme == "https"
	if !tls && u.Scheme != "http" {
		return "", false, fmt.Errorf("unsupported scheme %q", u.Scheme)
	}
	port := u.Port()
	if port == "" {
		port = "443"
		if !tls {
			port = "80"
		}
	}
	return net.JoinHostPort(u.Hostname(), port), tls, nil
}

func randomHex(n int) (string, error) {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}

// The library only ever runs NetBird in userspace, so say so before any
// dial, not just when embed.New runs. On Android, NetBird's dialer otherwise
// expects a VpnService socket protector that this app never sets, and every
// dial to the management server fails until the 30 s deadline: sign-in on a
// fresh start hit exactly that, while setup-key starts (embed.New first)
// worked.
func init() {
	_ = os.Setenv(netstack.EnvUseNetstackMode, "true")
	_ = os.Setenv(netstack.EnvSkipProxy, "true")
}

func mgmClient(ctx context.Context, o loginOptions, key wgtypes.Key, st *steps) (*mgm.GrpcClient, error) {
	addr, tls, err := mgmAddr(o.ManagementURL)
	if err != nil {
		return nil, fmt.Errorf("management URL: %w", err)
	}
	if err := probe(ctx, addr, tls); err != nil {
		return nil, err
	}
	st.done("probe")
	c, err := mgm.NewClient(ctx, addr, key, tls)
	if err == nil {
		st.done("grpc connect")
	}
	return c, err
}

// probe checks the management address the way gRPC will use it. NetBird's
// client dials with retries until a deadline and then only says "context
// deadline exceeded"; this names the step that fails (name, port, TLS,
// HTTP/2) so a wrong URL or proxy setup is visible.
func probe(ctx context.Context, addr string, useTLS bool) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	var d net.Dialer
	conn, err := d.DialContext(ctx, "tcp", addr)
	if err != nil {
		return fmt.Errorf("cannot reach the management server at %s: %w", addr, err)
	}
	defer conn.Close()
	if !useTLS {
		return nil
	}
	host, _, _ := net.SplitHostPort(addr)
	roots, _ := x509.SystemCertPool()
	tc := tls.Client(conn, &tls.Config{ServerName: host, RootCAs: roots, NextProtos: []string{"h2"}})
	if err := tc.HandshakeContext(ctx); err != nil {
		return fmt.Errorf("TLS with the management server at %s failed: %w", addr, err)
	}
	if p := tc.ConnectionState().NegotiatedProtocol; p != "h2" {
		return fmt.Errorf("the server at %s does not offer HTTP/2, which NetBird's gRPC needs: "+
			"check the management URL and port, or that the reverse proxy passes gRPC through", addr)
	}
	return nil
}

func beginLogin(o loginOptions) (*pendingLogin, string, error) {
	st := newSteps()
	var key wgtypes.Key
	var err error
	if o.PrivateKey != "" {
		key, err = wgtypes.ParseKey(o.PrivateKey)
	} else {
		key, err = wgtypes.GeneratePrivateKey()
	}
	if err != nil {
		return nil, "", fmt.Errorf("peer key: %w", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	client, err := mgmClient(ctx, o, key, st)
	if err != nil {
		return nil, "", err
	}
	defer client.Close()
	flow, err := client.GetPKCEAuthorizationFlow(false)
	if err != nil {
		return nil, "", fmt.Errorf("this NetBird server has no SSO sign-in: %w", err)
	}
	st.done("sso settings")
	cfg := flow.GetProviderConfig()
	if cfg.GetAuthorizationEndpoint() == "" || cfg.GetTokenEndpoint() == "" || cfg.GetClientID() == "" {
		return nil, "", errors.New("this NetBird server's SSO settings are incomplete")
	}

	// The first redirect URL whose localhost port is free, as NetBird does.
	var listener net.Listener
	var redirect string
	var busy []string
	for _, r := range cfg.GetRedirectURLs() {
		u, err := url.Parse(r)
		if err != nil || u.Port() == "" {
			continue
		}
		if h := u.Hostname(); h != "localhost" && h != "127.0.0.1" {
			continue
		}
		l, err := net.Listen("tcp", net.JoinHostPort("127.0.0.1", u.Port()))
		if err != nil {
			busy = append(busy, u.Port())
			continue
		}
		listener, redirect = l, r
		break
	}
	if listener == nil {
		if len(busy) > 0 {
			return nil, "", fmt.Errorf("localhost port %s for the sign-in is in use on this phone "+
				"(another app, or an unfinished sign-in): close it or restart Labterm", strings.Join(busy, ", "))
		}
		return nil, "", fmt.Errorf("the NetBird server lists no http://localhost:<port> redirect URL "+
			"for sign-in (it sent: %q); add http://localhost:53000 to its PKCE settings",
			cfg.GetRedirectURLs())
	}

	state, err := randomHex(24)
	if err != nil {
		listener.Close()
		return nil, "", err
	}
	verifier, err := randomHex(64)
	if err != nil {
		listener.Close()
		return nil, "", err
	}
	sum := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(sum[:])

	p := &pendingLogin{
		opts: o,
		key:  key,
		oauth: oauth2.Config{
			ClientID:     cfg.GetClientID(),
			ClientSecret: cfg.GetClientSecret(), //nolint:staticcheck
			Endpoint: oauth2.Endpoint{
				AuthURL:  cfg.GetAuthorizationEndpoint(),
				TokenURL: cfg.GetTokenEndpoint(),
			},
			RedirectURL: redirect,
			Scopes:      scopes(cfg.GetScope()),
		},
		useID:    cfg.GetUseIDToken(),
		state:    state,
		verifier: verifier,
		codes:    make(chan string, 1),
		errs:     make(chan error, 1),
	}
	params := []oauth2.AuthCodeOption{
		oauth2.SetAuthURLParam("code_challenge_method", "S256"),
		oauth2.SetAuthURLParam("code_challenge", challenge),
	}
	if a := cfg.GetAudience(); a != "" {
		params = append(params, oauth2.SetAuthURLParam("audience", a))
	}
	if a := strings.TrimSpace(o.Account); a != "" {
		// Ask for this user, not whoever the browser is signed in as.
		params = append(params,
			oauth2.SetAuthURLParam("login_hint", a),
			oauth2.SetAuthURLParam("prompt", "login"))
	}
	authURL := p.oauth.AuthCodeURL(state, params...)

	cbPath := "/"
	if u, err := url.Parse(redirect); err == nil && u.Path != "" {
		cbPath = u.Path
	}
	mux := http.NewServeMux()
	mux.HandleFunc(cbPath, func(w http.ResponseWriter, r *http.Request) {
		q := r.URL.Query()
		var err error
		switch {
		case q.Get("error") != "":
			err = fmt.Errorf("sign-in failed: %s %s", q.Get("error"), q.Get("error_description"))
		case subtle.ConstantTimeCompare([]byte(q.Get("state")), []byte(p.state)) != 1:
			http.Error(w, "invalid state", http.StatusBadRequest)
			return
		case q.Get("code") == "":
			err = errors.New("sign-in failed: no code")
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		if err != nil {
			select {
			case p.errs <- err:
			default:
			}
			fmt.Fprint(w, page("Sign-in failed. Go back to Labterm and try again."))
			return
		}
		select {
		case p.codes <- q.Get("code"):
		default:
		}
		fmt.Fprint(w, page("Signed in. You can go back to Labterm."))
	})
	p.server = &http.Server{Handler: mux, ReadHeaderTimeout: 10 * time.Second}
	if l6, err := net.Listen("tcp", net.JoinHostPort("::1", listenerPort(listener))); err == nil {
		p.listener6 = l6
		go func() { _ = p.server.Serve(l6) }()
	}
	go func() { _ = p.server.Serve(listener) }()
	st.done("listen")
	p.timings = st.String()
	return p, authURL, nil
}

func listenerPort(l net.Listener) string {
	_, port, _ := net.SplitHostPort(l.Addr().String())
	return port
}

func scopes(s string) []string {
	if f := strings.Fields(s); len(f) > 0 {
		return f
	}
	return []string{"openid"}
}

func page(msg string) string {
	return `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Labterm</title><p style="font:16px system-ui;margin:2em">` + msg + `</p>`
}

func finishLogin(p *pendingLogin, timeout time.Duration) (loginResult, error) {
	defer p.close()
	st := newSteps()
	var code string
	select {
	case code = <-p.codes:
	case err := <-p.errs:
		return loginResult{}, err
	case <-time.After(timeout):
		return loginResult{}, errors.New("sign-in timed out")
	}
	st.done("browser")
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	tok, err := p.oauth.Exchange(ctx, code, oauth2.SetAuthURLParam("code_verifier", p.verifier))
	if err != nil {
		return loginResult{}, fmt.Errorf("sign-in: %w", err)
	}
	st.done("token")
	claims := idClaims(tok)
	if !isAccount(p.opts.Account, claims) {
		// Checked before Register so the wrong user never gets this peer.
		who := claims.PreferredUsername
		if who == "" {
			who = claims.Email
		}
		if who == "" {
			who = "someone else"
		}
		return loginResult{}, fmt.Errorf("signed in as %s, but this network uses the account %s",
			who, strings.TrimSpace(p.opts.Account))
	}
	jwt := tok.AccessToken
	if p.useID {
		id, _ := tok.Extra("id_token").(string)
		if id == "" {
			return loginResult{}, errors.New("sign-in: the identity provider sent no ID token")
		}
		jwt = id
	}

	client, err := mgmClient(ctx, p.opts, p.key, st)
	if err != nil {
		return loginResult{}, err
	}
	defer client.Close()
	// nolint:staticcheck
	info := system.GetInfo(context.WithValue(ctx, system.DeviceNameCtxKey, p.opts.DeviceName))
	// Register both enrolls a new key and re-authorizes one whose login
	// expired; the server ties the peer to the signed-in user.
	if _, err := client.Register("", jwt, info, nil, nil); err != nil {
		return loginResult{}, fmt.Errorf("netbird: %w", err)
	}
	st.done("register")
	return loginResult{PrivateKey: p.key.String(), Email: claims.Email, Timings: st.String()}, nil
}

type tokenClaims struct {
	Email             string `json:"email"`
	PreferredUsername string `json:"preferred_username"`
}

// idClaims reads the ID token that came straight from the token endpoint.
// Its signature is not checked here: the management server checks the
// token it is given, and these claims only show who signed in and catch
// the wrong account before registering.
func idClaims(tok *oauth2.Token) tokenClaims {
	var claims tokenClaims
	id, _ := tok.Extra("id_token").(string)
	parts := strings.Split(id, ".")
	if len(parts) != 3 {
		return claims
	}
	raw, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return claims
	}
	_ = json.Unmarshal(raw, &claims)
	return claims
}

// isAccount is the rule of labterm-oidc-cert's ProviderConfig::is_account:
// preferred_username, email, or the email's local part, ignoring case.
// The local part matches any domain; it only guards against signing in
// as the wrong account, and the management server decides who gets in.
func isAccount(account string, c tokenClaims) bool {
	want := strings.ToLower(strings.TrimSpace(account))
	if want == "" {
		return true
	}
	email := strings.ToLower(c.Email)
	if strings.ToLower(c.PreferredUsername) == want || email == want {
		return true
	}
	local, _, found := strings.Cut(email, "@")
	return found && local == want
}

// tnb_login_begin starts an SSO sign-in. On success it stores the handle in
// *handle and returns a JSON loginBegin with the URL to open in a browser;
// on failure *handle stays 0 and the string is the error. Free with tnb_free.
//
//export tnb_login_begin
func tnb_login_begin(optsJSON *C.char, handle *C.uintptr_t) *C.char {
	*handle = 0
	var o loginOptions
	if err := json.Unmarshal([]byte(C.GoString(optsJSON)), &o); err != nil {
		return cstr("bad options: " + err.Error())
	}
	p, authURL, err := beginLogin(o)
	if err != nil {
		return cstr(err.Error())
	}
	out, _ := json.Marshal(loginBegin{AuthURL: authURL, Timings: p.timings})
	loginsMu.Lock()
	nextLogin++
	logins[nextLogin] = p
	*handle = C.uintptr_t(nextLogin)
	loginsMu.Unlock()
	return cstr(string(out))
}

func takeLogin(h C.uintptr_t, remove bool) *pendingLogin {
	loginsMu.Lock()
	defer loginsMu.Unlock()
	p := logins[uint64(h)]
	if remove {
		delete(logins, uint64(h))
	}
	return p
}

// tnb_login_finish waits for the browser to come back, registers the peer
// and frees the handle. *ok is 1 when the result is a JSON loginResult and
// 0 when it is an error.
//
//export tnb_login_finish
func tnb_login_finish(h C.uintptr_t, timeoutMs C.int, ok *C.int) *C.char {
	*ok = 0
	p := takeLogin(h, false)
	if p == nil {
		return cstr("no sign-in")
	}
	defer takeLogin(h, true)
	p.finishing.Store(true)
	timeout := time.Duration(timeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = 5 * time.Minute
	}
	res, err := finishLogin(p, timeout)
	if err != nil {
		return cstr(err.Error())
	}
	out, _ := json.Marshal(res)
	*ok = 1
	return cstr(string(out))
}

// tnb_login_cancel stops a sign-in that tnb_login_finish is waiting for.
//
//export tnb_login_cancel
func tnb_login_cancel(h C.uintptr_t) {
	p := takeLogin(h, false)
	if p == nil {
		return
	}
	select {
	case p.errs <- errors.New("sign-in cancelled"):
	default:
	}
	if !p.finishing.Load() {
		takeLogin(h, true)
		p.close()
	}
}
