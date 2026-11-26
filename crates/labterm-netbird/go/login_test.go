package main

import (
	"context"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/netbirdio/netbird/client/iface/netstack"
)

// Sign-in dials the management server before any embed.New; on Android
// that dial needs netstack mode or it waits for a socket protector.
func TestNetstackModeIsSetAtLoad(t *testing.T) {
	if !netstack.IsEnabled() {
		t.Fatal("netstack mode not enabled at load")
	}
}

func TestIsAccount(t *testing.T) {
	for _, c := range []struct {
		account string
		claims  tokenClaims
		want    bool
	}{
		{"", tokenClaims{}, true},
		{"mke-adm", tokenClaims{PreferredUsername: "MKE-adm"}, true},
		{" mke-adm ", tokenClaims{Email: "mke-adm@example.com"}, true},
		{"mke-adm@example.com", tokenClaims{Email: "Mke-Adm@example.com"}, true},
		{"mke-adm", tokenClaims{PreferredUsername: "mke", Email: "mke@example.com"}, false},
		{"mke-adm", tokenClaims{Email: "mke-adm.example.com"}, false},
		{"mke-adm", tokenClaims{}, false},
	} {
		if got := isAccount(c.account, c.claims); got != c.want {
			t.Errorf("isAccount(%q, %+v) = %v", c.account, c.claims, got)
		}
	}
}

func TestProbeNamesTheFailingStep(t *testing.T) {
	ctx := context.Background()
	// Nothing listening.
	l, _ := net.Listen("tcp", "127.0.0.1:0")
	addr := l.Addr().String()
	l.Close()
	if err := probe(ctx, addr, false); err == nil || !strings.Contains(err.Error(), "cannot reach") {
		t.Errorf("closed port: %v", err)
	}
	// TLS without HTTP/2, like a proxy that only serves the dashboard.
	srv := httptest.NewUnstartedServer(http.NotFoundHandler())
	srv.EnableHTTP2 = false
	srv.StartTLS()
	defer srv.Close()
	err := probe(ctx, srv.Listener.Addr().String(), true)
	if err == nil || !strings.Contains(err.Error(), "TLS") {
		// Self-signed: the handshake fails first, which names TLS.
		t.Errorf("self-signed TLS: %v", err)
	}
}
