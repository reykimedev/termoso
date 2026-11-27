// Command e2e-peer is the far end of the labterm-netbird end-to-end test: a
// NetBird peer that listens on :2222 inside the network and forwards each
// connection to -forward (an SSH server), or echoes when -forward is empty.
// It prints its NetBird IP on stdout once up. With -provider tailscale it is
// a tsnet node of a Tailscale or Headscale network instead (-management is
// the control server). It also answers DNS on its network IP (port 5353 on
// NetBird, 53 on Tailscale, whose split DNS takes no port):
// peer.home.example.com is itself, lan.home.example.com is -dns-lan. Not
// shipped in any app.
package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"os/signal"
	"strings"
	"time"

	"github.com/miekg/dns"
	"github.com/netbirdio/netbird/client/embed"
	"tailscale.com/tsnet"
)

func main() {
	mgmt := flag.String("management", "", "management URL")
	forward := flag.String("forward", "", "host:port to forward to; empty = echo")
	provider := flag.String("provider", "netbird", "netbird or tailscale")
	dnsLAN := flag.String("dns-lan", "192.0.2.10", "address of lan.home.example.com")
	flag.Parse()
	if *provider == "tailscale" {
		tailscalePeer(*mgmt, *forward, *dnsLAN)
		return
	}
	c, err := embed.New(embed.Options{
		DeviceName:    "e2e-peer",
		SetupKey:      os.Getenv("NB_SETUP_KEY"),
		ManagementURL: *mgmt,
		LogLevel:      "warn",
	})
	if err != nil {
		log.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	if err := c.Start(ctx); err != nil {
		log.Fatal(err)
	}
	ln, err := c.ListenTCP(":2222")
	if err != nil {
		log.Fatal(err)
	}
	st, err := c.Status()
	if err != nil {
		log.Fatal(err)
	}
	ip := strings.Split(st.LocalPeerState.IP, "/")[0]
	pc, err := c.ListenUDP(net.JoinHostPort(ip, "5353"))
	if err != nil {
		log.Fatal(err)
	}
	go serveDNS(pc, ip, *dnsLAN)
	fmt.Println(ip)
	fmt.Println(st.LocalPeerState.FQDN)
	go func() {
		for {
			conn, err := ln.Accept()
			if err != nil {
				return
			}
			go serve(conn, *forward)
		}
	}()
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)
	<-sig
	_ = c.Stop(context.Background())
}

func tailscalePeer(control, forward, dnsLAN string) {
	dir, err := os.MkdirTemp("", "e2e-tsnet")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(dir)
	srv := &tsnet.Server{
		Dir:        dir,
		Hostname:   "e2e-peer",
		AuthKey:    os.Getenv("NB_SETUP_KEY"),
		ControlURL: control,
		Logf:       func(string, ...any) {},
	}
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	st, err := srv.Up(ctx)
	if err != nil {
		log.Fatal(err)
	}
	ln, err := srv.Listen("tcp", ":2222")
	if err != nil {
		log.Fatal(err)
	}
	ip := st.TailscaleIPs[0].String()
	pc, err := srv.ListenPacket("udp", net.JoinHostPort(ip, "53"))
	if err != nil {
		log.Fatal(err)
	}
	go serveDNS(pc, ip, dnsLAN)
	fmt.Println(ip)
	fmt.Println(strings.TrimSuffix(st.Self.DNSName, "."))
	go func() {
		for {
			conn, err := ln.Accept()
			if err != nil {
				return
			}
			go serve(conn, forward)
		}
	}()
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)
	<-sig
	_ = srv.Close()
}

func serve(conn net.Conn, forward string) {
	defer conn.Close()
	if forward == "" {
		_, _ = io.Copy(conn, conn)
		return
	}
	up, err := net.Dial("tcp", forward)
	if err != nil {
		return
	}
	defer up.Close()
	go func() { _, _ = io.Copy(up, conn) }()
	_, _ = io.Copy(conn, up)
}

// serveDNS answers A queries for the two test names; anything else is
// NXDOMAIN, so a lookup that reaches this server for another name fails.
func serveDNS(pc net.PacketConn, self, lan string) {
	buf := make([]byte, 1500)
	for {
		n, from, err := pc.ReadFrom(buf)
		if err != nil {
			return
		}
		q := new(dns.Msg)
		if q.Unpack(buf[:n]) != nil || len(q.Question) != 1 {
			continue
		}
		r := new(dns.Msg)
		r.SetReply(q)
		answer := map[string]string{
			"peer.home.example.com.": self,
			"lan.home.example.com.":  lan,
		}[strings.ToLower(q.Question[0].Name)]
		switch {
		case answer == "":
			r.Rcode = dns.RcodeNameError
		case q.Question[0].Qtype == dns.TypeA:
			r.Answer = append(r.Answer, &dns.A{
				Hdr: dns.RR_Header{Name: q.Question[0].Name, Rrtype: dns.TypeA, Class: dns.ClassINET, Ttl: 60},
				A:   net.ParseIP(answer),
			})
		}
		out, err := r.Pack()
		if err == nil {
			_, _ = pc.WriteTo(out, from)
		}
	}
}
