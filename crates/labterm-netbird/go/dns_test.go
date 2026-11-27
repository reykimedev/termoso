package main

import (
	"net/netip"
	"testing"

	mgmProto "github.com/netbirdio/netbird/shared/management/proto"
)

func TestPickServers(t *testing.T) {
	lan := netip.MustParseAddrPort("192.0.2.1:53")
	outside := netip.MustParseAddrPort("198.51.100.53:53")
	overlay := netip.MustParseAddrPort("100.64.0.9:53")
	tunnel := []netip.Prefix{
		netip.MustParsePrefix("100.64.0.0/10"),
		netip.MustParsePrefix("192.0.2.0/24"),
	}
	groups := []nsGroup{
		{Servers: []netip.AddrPort{outside}},
		{Servers: []netip.AddrPort{lan}, Domains: []string{"home.example.com"}},
		{Servers: []netip.AddrPort{overlay}, Domains: []string{"lab.home.example.com"}},
		{Servers: []netip.AddrPort{outside}, Domains: []string{"public.example.com"}},
	}
	cases := []struct {
		name string
		want []netip.AddrPort
	}{
		{"nas.home.example.com", []netip.AddrPort{lan}},
		{"NAS.Home.Example.com.", []netip.AddrPort{lan}},
		{"home.example.com", []netip.AddrPort{lan}},
		{"pve.lab.home.example.com", []netip.AddrPort{overlay}},
		{"nothome.example.com", nil}, // primary only, and it is outside the tunnel
		{"www.public.example.com", nil},
		{"peer.netbird.example", nil}, // peer names stay with NetBird
		{"nas", nil},
	}
	for _, c := range cases {
		got, _ := pickServers(c.name, "netbird.example", groups, tunnel)
		if len(got) != len(c.want) || (len(got) > 0 && got[0] != c.want[0]) {
			t.Errorf("%s: got %v, want %v", c.name, got, c.want)
		}
	}
	// A primary group inside the tunnel answers everything else.
	primary := append([]nsGroup{{Servers: []netip.AddrPort{lan}}}, groups[1:]...)
	if got, matched := pickServers("www.example.org", "netbird.example", primary, tunnel); len(got) != 1 || got[0] != lan || !matched {
		t.Errorf("primary: got %v", got)
	}
	// Covered by a group whose server isn't reachable (yet): matched, so a
	// fresh client waits for the route.
	if got, matched := pickServers("www.public.example.com", "netbird.example", groups, tunnel); got != nil || !matched {
		t.Errorf("unreachable group: got %v %v", got, matched)
	}
	if _, matched := pickServers("peer.netbird.example", "netbird.example", groups, tunnel); matched {
		t.Error("peer names must not match")
	}
}

func TestPeerDomain(t *testing.T) {
	if d := peerDomain("laptop.NetBird.example."); d != "netbird.example" {
		t.Errorf("got %q", d)
	}
	if d := peerDomain(""); d != "" {
		t.Errorf("got %q", d)
	}
}

func TestTunnelPrefixes(t *testing.T) {
	got := tunnelPrefixes("100.64.3.4/16", [][]string{{"100.64.0.7", "192.0.2.0/24", "nas.example.com"}})
	want := []string{"100.64.0.0/16", "100.64.0.7/32", "192.0.2.0/24"}
	if len(got) != len(want) {
		t.Fatalf("got %v", got)
	}
	for i := range want {
		if got[i].String() != want[i] {
			t.Errorf("got %v, want %v", got, want)
		}
	}
}

func TestSearchList(t *testing.T) {
	cfg := &mgmProto.DNSConfig{
		CustomZones: []*mgmProto.CustomZone{
			{Domain: "netbird.example."},
			{Domain: "zone.example.com.", SearchDomainDisabled: true},
			{Domain: "lab.example.com."},
		},
		NameServerGroups: []*mgmProto.NameServerGroup{
			{Domains: []string{"home.example.com"}, SearchDomainsEnabled: true},
			{Domains: []string{"office.example.com"}},
		},
	}
	got := searchList("netbird.example", cfg)
	want := []string{"netbird.example", "lab.example.com", "home.example.com"}
	if len(got) != len(want) {
		t.Fatalf("got %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v, want %v", got, want)
		}
	}
}
