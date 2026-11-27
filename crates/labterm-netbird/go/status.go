package main

/*
#include <stdint.h>
*/
import "C"

import (
	"encoding/json"
	"sort"
	"time"

	"github.com/netbirdio/netbird/client/embed"
)

// statusOut is what the network's details page shows: this peer's address
// and the peers it can see. No keys and no traffic contents.
type statusOut struct {
	IP              string    `json:"ip,omitempty"`
	FQDN            string    `json:"fqdn,omitempty"`
	Management      bool      `json:"management"`
	ManagementError string    `json:"management_error,omitempty"`
	Signal          bool      `json:"signal"`
	Peers           []peerOut `json:"peers"`
	// NetBird's nameserver groups as this peer applied them.
	DNS []dnsGroupOut `json:"dns,omitempty"`
}

type dnsGroupOut struct {
	// Match domains; empty = primary (all other names).
	Domains []string `json:"domains,omitempty"`
	Servers []string `json:"servers"`
	// Some server is reachable through the network, so Labterm asks it
	// itself (see dns.go); otherwise NetBird's resolver does.
	Tunnel bool `json:"tunnel"`
}

type peerOut struct {
	FQDN      string `json:"fqdn,omitempty"`
	IP        string `json:"ip,omitempty"`
	Connected bool   `json:"connected"`
	Relayed   bool   `json:"relayed"`
	LatencyMs int64  `json:"latency_ms,omitempty"`
	// Unix seconds of the last WireGuard handshake; 0 = never.
	Handshake int64 `json:"handshake,omitempty"`
	// Networks this device reaches through the peer (a routing peer or an
	// exit node), e.g. "192.0.2.0/24" or "0.0.0.0/0".
	Routes []string `json:"routes,omitempty"`
}

func nbStatus(c *embed.Client) (statusOut, error) {
	st, err := c.Status()
	if err != nil {
		return statusOut{}, err
	}
	out := statusOut{
		IP:         st.LocalPeerState.IP,
		FQDN:       st.LocalPeerState.FQDN,
		Management: st.ManagementState.Connected,
		Signal:     st.SignalState.Connected,
		Peers:      make([]peerOut, 0, len(st.Peers)),
	}
	if e := st.ManagementState.Error; e != nil {
		out.ManagementError = e.Error()
	}
	for _, p := range st.Peers {
		po := peerOut{
			FQDN:      p.FQDN,
			IP:        p.IP,
			Connected: p.ConnStatus.String() == "Connected",
			Relayed:   p.Relayed,
			LatencyMs: p.Latency.Milliseconds(),
		}
		if !p.LastWireguardHandshake.IsZero() && p.LastWireguardHandshake.After(time.Unix(0, 0)) {
			po.Handshake = p.LastWireguardHandshake.Unix()
		}
		for r := range p.GetRoutes() {
			po.Routes = append(po.Routes, r)
		}
		sort.Strings(po.Routes)
		out.Peers = append(out.Peers, po)
	}
	sort.Slice(out.Peers, func(i, j int) bool { return out.Peers[i].FQDN < out.Peers[j].FQDN })
	var rows [][]string
	for _, p := range out.Peers {
		if p.Connected {
			rows = append(rows, append([]string{p.IP}, p.Routes...))
		}
	}
	tunnel := tunnelPrefixes(st.LocalPeerState.IP, rows)
	for _, g := range st.NSGroupStates {
		d := dnsGroupOut{Domains: g.Domains, Servers: make([]string, 0, len(g.Servers))}
		for _, sv := range g.Servers {
			d.Servers = append(d.Servers, sv.String())
			if inTunnel(sv.Addr(), tunnel) {
				d.Tunnel = true
			}
		}
		out.DNS = append(out.DNS, d)
	}
	return out, nil
}

// tnb_status returns the node's state as a JSON statusOut. *ok is 1 on
// success and 0 when the string is an error. Free with tnb_free.
//
//export tnb_status
func tnb_status(h C.uintptr_t, ok *C.int) *C.char {
	*ok = 0
	n, found := lookup(h)
	if !found {
		return cstr("not running")
	}
	st, err := n.client.Status()
	if err != nil {
		return cstr(err.Error())
	}
	out, _ := json.Marshal(st)
	*ok = 1
	return cstr(string(out))
}
