package main

import (
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
