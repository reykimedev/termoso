package main

/*
#include <stdint.h>
*/
import "C"

import (
	"encoding/json"
	"sort"
	"time"
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
}

type peerOut struct {
	FQDN      string `json:"fqdn,omitempty"`
	IP        string `json:"ip,omitempty"`
	Connected bool   `json:"connected"`
	Relayed   bool   `json:"relayed"`
	LatencyMs int64  `json:"latency_ms,omitempty"`
	// Unix seconds of the last WireGuard handshake; 0 = never.
	Handshake int64 `json:"handshake,omitempty"`
}

func nodeStatus(n *node) (statusOut, error) {
	st, err := n.client.Status()
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
		out.Peers = append(out.Peers, po)
	}
	sort.Slice(out.Peers, func(i, j int) bool { return out.Peers[i].FQDN < out.Peers[j].FQDN })
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
	st, err := nodeStatus(n)
	if err != nil {
		return cstr(err.Error())
	}
	out, _ := json.Marshal(st)
	*ok = 1
	return cstr(string(out))
}
