package main

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/netip"
	"strings"
	"time"

	"github.com/miekg/dns"
	"github.com/netbirdio/netbird/client/embed"
	log "github.com/sirupsen/logrus"
)

// NetBird's own resolver answers peer names, but in netstack mode it asks
// the nameserver groups' upstream servers over the host network: a
// nameserver that is only reachable through a NetBird route (a home
// firewall's LAN address) never answers. So for names a nameserver group
// covers, Labterm asks that group's servers itself, through the tunnel, and
// leaves everything else (and every failure) to NetBird's resolver. Nothing
// here touches the system's DNS.

// dnsTimeout bounds one lookup; a failed lookup falls back to NetBird.
const dnsTimeout = 3 * time.Second

// dnsSettle is how long after start a lookup waits for the nameserver
// groups and routes to show up: the first sync lands in the status a moment
// after the client is up, and a host opened right away would miss them.
const dnsSettle = 5 * time.Second

// nsGroup is one nameserver group as the client applied it: Domains is
// empty for a primary group (all names).
type nsGroup struct {
	Servers []netip.AddrPort
	Domains []string
}

// tunnelPrefixes are the networks a dial reaches through NetBird: the
// overlay and every route a connected peer serves (an exit node's default
// route included). Peers come as IP and routes, already filtered to
// connected ones.
func tunnelPrefixes(localIP string, peers [][]string) []netip.Prefix {
	var out []netip.Prefix
	if p, err := netip.ParsePrefix(localIP); err == nil {
		out = append(out, p.Masked())
	}
	for _, p := range peers {
		for i, r := range p {
			if i == 0 {
				if a, err := netip.ParseAddr(r); err == nil {
					out = append(out, netip.PrefixFrom(a, a.BitLen()))
				}
				continue
			}
			// Domain routes are names, not prefixes: skip them.
			if pr, err := netip.ParsePrefix(r); err == nil {
				out = append(out, pr.Masked())
			}
		}
	}
	return out
}

func inTunnel(a netip.Addr, prefixes []netip.Prefix) bool {
	a = a.Unmap()
	for _, p := range prefixes {
		if p.Contains(a) {
			return true
		}
	}
	return false
}

// underDomain reports whether name is domain or a name below it.
func underDomain(name, domain string) bool {
	domain = strings.TrimSuffix(strings.ToLower(domain), ".")
	if domain == "" {
		return false
	}
	return name == domain || strings.HasSuffix(name, "."+domain)
}

// pickServers returns the tunnel-reachable servers of the group that
// answers name: the group with the longest matching domain, else the
// primary groups. Names in the peer domain (peer names) and names whose
// group has no server inside the tunnel get none: NetBird resolves those.
// matched reports that a group covers name.
func pickServers(name, peerDomain string, groups []nsGroup, tunnel []netip.Prefix) (servers []netip.AddrPort, matched bool) {
	name = strings.TrimSuffix(strings.ToLower(name), ".")
	if !strings.Contains(name, ".") || underDomain(name, peerDomain) {
		return nil, false
	}
	best, bestLen := -1, 0
	for i, g := range groups {
		for _, d := range g.Domains {
			if underDomain(name, d) && len(d) > bestLen {
				best, bestLen = i, len(d)
			}
		}
	}
	var chosen []nsGroup
	if best >= 0 {
		chosen = groups[best : best+1]
	} else {
		for _, g := range groups {
			if len(g.Domains) == 0 {
				chosen = append(chosen, g)
			}
		}
	}
	for _, g := range chosen {
		for _, s := range g.Servers {
			if inTunnel(s.Addr(), tunnel) {
				servers = append(servers, s)
			}
		}
	}
	return servers, len(chosen) > 0
}

// peerDomain is the account's DNS domain, from this peer's FQDN.
func peerDomain(fqdn string) string {
	fqdn = strings.TrimSuffix(fqdn, ".")
	if _, d, ok := strings.Cut(fqdn, "."); ok {
		return strings.ToLower(d)
	}
	return ""
}

// tunnelLookup asks servers for name's A and AAAA records over dial (UDP
// through the tunnel). IPv4 answers come first.
func tunnelLookup(ctx context.Context, dial func(ctx context.Context, network, addr string) (net.Conn, error), name string, servers []netip.AddrPort) ([]netip.Addr, netip.AddrPort, error) {
	fq := dns.Fqdn(name)
	var lastErr error
	for _, s := range servers {
		var v4, v6 []netip.Addr
		ok := false
		for _, qt := range []uint16{dns.TypeA, dns.TypeAAAA} {
			addrs, err := exchange(ctx, dial, s, fq, qt)
			if err != nil {
				lastErr = err
				continue
			}
			ok = true
			if qt == dns.TypeA {
				v4 = addrs
			} else {
				v6 = addrs
			}
		}
		if all := append(v4, v6...); len(all) > 0 {
			return all, s, nil
		}
		if ok {
			return nil, s, fmt.Errorf("%s: no address", name)
		}
	}
	if lastErr == nil {
		lastErr = errors.New("no nameserver")
	}
	return nil, netip.AddrPort{}, lastErr
}

func exchange(ctx context.Context, dial func(ctx context.Context, network, addr string) (net.Conn, error), server netip.AddrPort, fq string, qt uint16) ([]netip.Addr, error) {
	ctx, cancel := context.WithTimeout(ctx, dnsTimeout)
	defer cancel()
	raw, err := dial(ctx, "udp", server.String())
	if err != nil {
		return nil, err
	}
	defer raw.Close()
	if d, ok := ctx.Deadline(); ok {
		_ = raw.SetDeadline(d)
	}
	co := &dns.Conn{Conn: raw}
	m := new(dns.Msg)
	m.SetQuestion(fq, qt)
	if err := co.WriteMsg(m); err != nil {
		return nil, err
	}
	r, err := co.ReadMsg()
	if err != nil {
		return nil, err
	}
	if r.Id != m.Id {
		return nil, errors.New("dns: id mismatch")
	}
	if r.Rcode != dns.RcodeSuccess {
		return nil, fmt.Errorf("dns: %s", dns.RcodeToString[r.Rcode])
	}
	var out []netip.Addr
	for _, rr := range r.Answer {
		switch v := rr.(type) {
		case *dns.A:
			if a, ok := netip.AddrFromSlice(v.A); ok {
				out = append(out, a.Unmap())
			}
		case *dns.AAAA:
			if a, ok := netip.AddrFromSlice(v.AAAA); ok {
				out = append(out, a)
			}
		}
	}
	return out, nil
}

// tunnelServers reads the client's status and picks the servers for host.
// settling is true while the status may still be missing the first sync.
func tunnelServers(c *embed.Client, host string) (servers []netip.AddrPort, settling bool) {
	st, err := c.Status()
	if err != nil {
		return nil, false
	}
	// FullStatus lives in an internal package: copy out what is needed.
	groups := make([]nsGroup, 0, len(st.NSGroupStates))
	for _, g := range st.NSGroupStates {
		groups = append(groups, nsGroup{Servers: g.Servers, Domains: g.Domains})
	}
	var peers [][]string
	for i := range st.Peers {
		p := &st.Peers[i]
		if p.ConnStatus.String() != "Connected" {
			continue
		}
		row := []string{p.IP}
		for r := range p.GetRoutes() {
			row = append(row, r)
		}
		peers = append(peers, row)
	}
	tunnel := tunnelPrefixes(st.LocalPeerState.IP, peers)
	servers, matched := pickServers(host, peerDomain(st.LocalPeerState.FQDN), groups, tunnel)
	return servers, len(groups) == 0 || (matched && len(servers) == 0)
}

// dialTrace records how a dial found its address, for the connection test.
type dialTrace struct {
	// Name is the name that resolved (a short name under its search domain).
	Name string `json:"name,omitempty"`
	// Via: "address" (no lookup), "nameserver" (a nameserver group through
	// the tunnel), "netbird" (NetBird's resolver: peer names, servers outside
	// the network) or "tailscale" (MagicDNS, split DNS).
	Via string `json:"via"`
	// Server is the nameserver that answered, for "nameserver".
	Server string `json:"server,omitempty"`
}

func (t *dialTrace) set(name, via, server string) {
	if t != nil {
		*t = dialTrace{Name: name, Via: via, Server: server}
	}
}

// nbDial dials addr in the NetBird network, resolving a name through the
// nameserver group that covers it when NetBird itself cannot reach it. A
// single-label name ("nas") is tried under each of the network's search
// domains, as NetBird's own client does. tr, when set, learns how.
func nbDial(ctx context.Context, b nbNode, network, addr string, tr *dialTrace) (net.Conn, error) {
	c := b.c
	host, port, err := net.SplitHostPort(addr)
	if err != nil {
		return c.Dial(ctx, network, addr)
	}
	if _, err := netip.ParseAddr(host); err == nil {
		tr.set(host, "address", "")
		return c.Dial(ctx, network, addr)
	}
	name := strings.TrimSuffix(host, ".")
	if strings.Contains(name, ".") || strings.HasSuffix(host, ".") {
		return dialName(ctx, b, network, host, port, tr)
	}
	var firstErr error
	for _, d := range b.searchDomains(ctx) {
		conn, err := dialName(ctx, b, network, name+"."+d, port, tr)
		if err == nil {
			return conn, nil
		}
		if firstErr == nil {
			firstErr = err
		}
		if ctx.Err() != nil {
			return nil, firstErr
		}
	}
	// No search domain answered: let NetBird try the bare name.
	tr.set(host, "netbird", "")
	conn, err := c.Dial(ctx, network, addr)
	if err != nil && firstErr != nil {
		return nil, firstErr
	}
	return conn, err
}

// dialName resolves one name (through a nameserver group in the tunnel, or
// NetBird's resolver) and dials it.
func dialName(ctx context.Context, b nbNode, network, host, port string, tr *dialTrace) (net.Conn, error) {
	c := b.c
	addr := net.JoinHostPort(host, port)
	servers, settling := tunnelServers(c, host)
	for len(servers) == 0 && settling && time.Since(b.started) < dnsSettle {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(200 * time.Millisecond):
		}
		servers, settling = tunnelServers(c, host)
	}
	if len(servers) == 0 {
		tr.set(host, "netbird", "")
		return c.Dial(ctx, network, addr)
	}
	ips, server, err := tunnelLookup(ctx, c.Dial, host, servers)
	if err != nil {
		log.Debugf("labterm dns: %s through the tunnel: %v; using NetBird's resolver", host, err)
		tr.set(host, "netbird", "")
		return c.Dial(ctx, network, addr)
	}
	tr.set(host, "nameserver", server.String())
	for _, ip := range ips {
		conn, derr := c.Dial(ctx, network, net.JoinHostPort(ip.String(), port))
		if derr == nil {
			return conn, nil
		}
		err = derr
		if ctx.Err() != nil {
			break
		}
	}
	return nil, err
}
