package main

import "testing"

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
