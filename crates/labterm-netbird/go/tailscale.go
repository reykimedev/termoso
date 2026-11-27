package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"tailscale.com/client/local"
	"tailscale.com/ipn"
	"tailscale.com/ipn/ipnstate"
	"tailscale.com/tsnet"
)

// Tailscale networks run on tsnet, Tailscale's own userspace node, in the
// same library (and Go runtime) as NetBird. Like NetBird: no system VPN,
// only the dials Labterm makes go through it. The node's state (machine and
// node keys) is the "peer key": it comes from and goes back to Labterm's
// encrypted device store, never to disk in the clear and never synced.

const providerTailscale = "tailscale"

func init() {
	// Never upload tsnet's logs to Tailscale's log server (the builds also
	// leave the log uploader out with -tags ts_omit_logtail).
	_ = os.Setenv("TS_NO_LOGS_NO_SUPPORT", "true")
}

// stateStore is an in-memory ipn.StateStore that Labterm loads and saves.
// tsnet refuses its own mem.Store for nodes that are not ephemeral.
type stateStore struct {
	mu sync.Mutex
	m  map[ipn.StateKey][]byte
}

func newStateStore(saved string) (*stateStore, error) {
	s := &stateStore{m: map[ipn.StateKey][]byte{}}
	if saved == "" {
		return s, nil
	}
	raw, err := base64.StdEncoding.DecodeString(saved)
	if err != nil {
		return nil, fmt.Errorf("saved tailscale state: %w", err)
	}
	if err := json.Unmarshal(raw, &s.m); err != nil {
		return nil, fmt.Errorf("saved tailscale state: %w", err)
	}
	return s, nil
}

func (s *stateStore) String() string { return "labterm" }

func (s *stateStore) ReadState(id ipn.StateKey) ([]byte, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	v, ok := s.m[id]
	if !ok {
		return nil, ipn.ErrStateNotExist
	}
	return append([]byte(nil), v...), nil
}

func (s *stateStore) WriteState(id ipn.StateKey, bs []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.m[id] = append([]byte(nil), bs...)
	return nil
}

func (s *stateStore) export() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	raw, _ := json.Marshal(s.m)
	return base64.StdEncoding.EncodeToString(raw)
}

func (s *stateStore) empty() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return len(s.m) == 0
}

var hostnameBad = regexp.MustCompile(`[^a-z0-9-]+`)

// tsHostname turns "Labterm Ops Google Pixel 9" into a valid machine name.
func tsHostname(name string) string {
	h := strings.Trim(hostnameBad.ReplaceAllString(strings.ToLower(name), "-"), "-")
	if len(h) > 63 {
		h = strings.TrimRight(h[:63], "-")
	}
	if h == "" {
		h = "labterm"
	}
	return h
}

// tsDir is tsnet's working directory: with our own store and no log
// uploads it holds nothing secret, but tsnet wants one. One per control
// server and network, under the app's cache.
func tsDir(dataDir, controlURL, key string) (string, error) {
	base := dataDir
	if base == "" {
		base = os.TempDir()
	}
	sum := sha256.Sum256([]byte(controlURL + "\x00" + key))
	dir := filepath.Join(base, "labterm-tailscale", hex.EncodeToString(sum[:8]))
	return dir, os.MkdirAll(dir, 0o700)
}

type tsNode struct {
	srv   *tsnet.Server
	lc    *local.Client
	store *stateStore
}

func newTsServer(o startOptions, store *stateStore, authKey string) (*tsNode, error) {
	dir, err := tsDir(o.DataDir, o.ManagementURL, o.NetworkID)
	if err != nil {
		return nil, err
	}
	srv := &tsnet.Server{
		Dir:        dir,
		Store:      store,
		Hostname:   tsHostname(o.DeviceName),
		AuthKey:    authKey,
		ControlURL: o.ManagementURL,
		// Quiet unless asked: tsnet logs every netmap.
		Logf:     func(string, ...any) {},
		UserLogf: func(string, ...any) {},
	}
	if o.LogLevel == "debug" || o.LogLevel == "trace" || os.Getenv("LABTERM_TS_DEBUG") != "" {
		srv.Logf = func(f string, a ...any) { fmt.Fprintf(os.Stderr, "tailscale: "+f+"\n", a...) }
	}
	if err := srv.Start(); err != nil {
		return nil, err
	}
	lc, err := srv.LocalClient()
	if err != nil {
		srv.Close()
		return nil, err
	}
	return &tsNode{srv: srv, lc: lc, store: store}, nil
}

// errTsNeedsLogin matches the Rust side's list of refused logins.
var errTsNeedsLogin = errors.New("login expired: tailscale needs a new sign-in")

// waitRunning waits until the node is up. A node that needs a login fails at
// once rather than waiting for a browser nobody opened.
func (t *tsNode) waitRunning(ctx context.Context) error {
	w, err := t.lc.WatchIPNBus(ctx, ipn.NotifyInitialState)
	if err != nil {
		return err
	}
	defer w.Close()
	for {
		n, err := w.Next()
		if err != nil {
			return err
		}
		if n.ErrMessage != nil {
			return errors.New(*n.ErrMessage)
		}
		if n.State != nil {
			switch *n.State {
			case ipn.Running:
				return nil
			case ipn.NeedsLogin:
				if t.srv.AuthKey == "" {
					return errTsNeedsLogin
				}
			case ipn.NeedsMachineAuth:
				return errors.New("tailscale: an admin must approve this device")
			}
		}
	}
}

// acceptRoutes lets the node use subnet routes other machines advertise, so
// plain LAN addresses work (like NetBird's network routes).
func (t *tsNode) acceptRoutes(ctx context.Context) error {
	_, err := t.lc.EditPrefs(ctx, &ipn.MaskedPrefs{
		Prefs:       ipn.Prefs{RouteAll: true},
		RouteAllSet: true,
	})
	return err
}

func startTailscale(o startOptions, timeout time.Duration) (*tsNode, startResult, error) {
	store, err := newStateStore(o.PrivateKey)
	if err != nil {
		return nil, startResult{}, err
	}
	if store.empty() && o.SetupKey == "" {
		return nil, startResult{}, errors.New("no setup key")
	}
	authKey := ""
	if store.empty() {
		authKey = o.SetupKey
	}
	try := func(authKey string) (*tsNode, error) {
		t, err := newTsServer(o, store, authKey)
		if err != nil {
			return nil, err
		}
		ctx, cancel := context.WithTimeout(context.Background(), timeout)
		defer cancel()
		if err := t.waitRunning(ctx); err != nil {
			t.srv.Close()
			return nil, err
		}
		if err := t.acceptRoutes(ctx); err != nil {
			t.srv.Close()
			return nil, err
		}
		return t, nil
	}
	t, err := try(authKey)
	if errors.Is(err, errTsNeedsLogin) && authKey == "" && o.SetupKey != "" {
		// The saved node is gone (removed, key expired): join anew.
		store, _ = newStateStore("")
		authKey = o.SetupKey
		t, err = try(authKey)
	}
	if err != nil {
		return nil, startResult{}, err
	}
	return t, startResult{PrivateKey: t.store.export(), Registered: authKey != ""}, nil
}

func (t *tsNode) Dial(ctx context.Context, network, addr string) (net.Conn, error) {
	return t.srv.Dial(ctx, network, addr)
}

func (t *tsNode) Stop(context.Context) error { return t.srv.Close() }

func (t *tsNode) Status() (statusOut, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	st, err := t.lc.Status(ctx)
	if err != nil {
		return statusOut{}, err
	}
	return tsStatus(st), nil
}

func tsStatus(st *ipnstate.Status) statusOut {
	out := statusOut{
		Management: st.BackendState == ipn.Running.String(),
		// Tailscale has no signal server; DERP plays that part.
		Signal: st.BackendState == ipn.Running.String(),
		Peers:  []peerOut{},
	}
	if len(st.Health) > 0 {
		out.ManagementError = strings.Join(st.Health, "; ")
	}
	if st.Self != nil {
		if len(st.Self.TailscaleIPs) > 0 {
			out.IP = st.Self.TailscaleIPs[0].String()
		}
		out.FQDN = strings.TrimSuffix(st.Self.DNSName, ".")
	}
	for _, p := range st.Peer {
		po := peerOut{
			FQDN:      strings.TrimSuffix(p.DNSName, "."),
			Connected: p.Active || p.CurAddr != "" || p.Relay != "" && p.Online,
			Relayed:   p.CurAddr == "" && p.Relay != "",
		}
		if len(p.TailscaleIPs) > 0 {
			po.IP = p.TailscaleIPs[0].String()
		}
		if !p.LastHandshake.IsZero() {
			po.Handshake = p.LastHandshake.Unix()
		}
		if p.PrimaryRoutes != nil {
			for _, r := range p.PrimaryRoutes.All() {
				po.Routes = append(po.Routes, r.String())
			}
		}
		if p.ExitNode {
			po.Routes = append(po.Routes, "0.0.0.0/0")
		}
		sort.Strings(po.Routes)
		out.Peers = append(out.Peers, po)
	}
	sort.Slice(out.Peers, func(i, j int) bool { return out.Peers[i].FQDN < out.Peers[j].FQDN })
	return out
}

// tsLogin is a browser sign-in to Tailscale (or Headscale): the node asks
// the control server for a login URL, the person signs in there, and the
// node comes up. Its state is the result.
type tsLogin struct {
	node *tsNode
	opts loginOptions
	ctx  context.Context
	stop context.CancelFunc
	errs chan error
	once sync.Once
	// Set while finish waits (see pendingLogin.finishing).
	waitingFlag atomic.Bool
}

func beginTsLogin(o loginOptions) (*tsLogin, string, error) {
	store, err := newStateStore(o.PrivateKey)
	if err != nil {
		return nil, "", err
	}
	t, err := newTsServer(startOptions{
		ManagementURL: o.ManagementURL,
		DeviceName:    o.DeviceName,
		DataDir:       o.DataDir,
		NetworkID:     o.NetworkID,
	}, store, "")
	if err != nil {
		return nil, "", err
	}
	ctx, cancel := context.WithCancel(context.Background())
	l := &tsLogin{node: t, opts: o, ctx: ctx, stop: cancel, errs: make(chan error, 1)}
	begin, bcancel := context.WithTimeout(ctx, 30*time.Second)
	defer bcancel()
	w, err := t.lc.WatchIPNBus(begin, ipn.NotifyInitialState)
	if err != nil {
		l.cancel()
		return nil, "", err
	}
	defer w.Close()
	if err := t.lc.StartLoginInteractive(begin); err != nil {
		l.cancel()
		return nil, "", err
	}
	for {
		n, err := w.Next()
		if err != nil {
			l.cancel()
			return nil, "", fmt.Errorf("tailscale: no sign-in page: %w", err)
		}
		if n.ErrMessage != nil {
			l.cancel()
			return nil, "", errors.New(*n.ErrMessage)
		}
		if n.BrowseToURL != nil && *n.BrowseToURL != "" {
			return l, *n.BrowseToURL, nil
		}
	}
}

func (l *tsLogin) cancel() {
	l.once.Do(func() {
		l.stop()
		l.node.srv.Close()
	})
}

func (l *tsLogin) abort(err error) {
	select {
	case l.errs <- err:
	default:
	}
	l.stop()
}

func (l *tsLogin) finish(timeout time.Duration) (loginResult, error) {
	l.waitingFlag.Store(true)
	defer l.cancel()
	ctx, cancel := context.WithTimeout(l.ctx, timeout)
	defer cancel()
	if err := l.node.waitRunning(ctx); err != nil {
		select {
		case e := <-l.errs:
			return loginResult{}, e
		default:
		}
		if errors.Is(err, errTsNeedsLogin) {
			// Not signed in yet: keep waiting for the browser.
			err = l.waitLoggedIn(ctx)
		}
		if err != nil {
			return loginResult{}, err
		}
	}
	st, err := l.node.lc.Status(ctx)
	if err != nil {
		return loginResult{}, err
	}
	login := ""
	if st.Self != nil {
		if u, ok := st.User[st.Self.UserID]; ok {
			login = u.LoginName
		}
	}
	if !isAccount(l.opts.Account, tokenClaims{Email: login}) {
		// Signed in as someone else: log that node out again so it joins
		// nothing, like the NetBird sign-in refuses to register.
		_ = l.node.lc.Logout(context.Background())
		return loginResult{}, fmt.Errorf("signed in as %s, not %s", login, l.opts.Account)
	}
	return loginResult{PrivateKey: l.node.store.export(), Email: login}, nil
}

func (l *tsLogin) waitLoggedIn(ctx context.Context) error {
	w, err := l.node.lc.WatchIPNBus(ctx, ipn.NotifyInitialState)
	if err != nil {
		return err
	}
	defer w.Close()
	for {
		n, err := w.Next()
		if err != nil {
			select {
			case e := <-l.errs:
				return e
			default:
			}
			return err
		}
		if n.State != nil && *n.State == ipn.Running {
			return nil
		}
	}
}
