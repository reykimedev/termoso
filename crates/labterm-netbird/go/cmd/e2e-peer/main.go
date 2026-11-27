// Command e2e-peer is the far end of the labterm-netbird end-to-end test: a
// NetBird peer that listens on :2222 inside the network and forwards each
// connection to -forward (an SSH server), or echoes when -forward is empty.
// It prints its NetBird IP on stdout once up. With -provider tailscale it is
// a tsnet node of a Tailscale or Headscale network instead (-management is
// the control server). Not shipped in any app.
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

	"github.com/netbirdio/netbird/client/embed"
	"tailscale.com/tsnet"
)

func main() {
	mgmt := flag.String("management", "", "management URL")
	forward := flag.String("forward", "", "host:port to forward to; empty = echo")
	provider := flag.String("provider", "netbird", "netbird or tailscale")
	flag.Parse()
	if *provider == "tailscale" {
		tailscalePeer(*mgmt, *forward)
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
	fmt.Println(strings.Split(st.LocalPeerState.IP, "/")[0])
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

func tailscalePeer(control, forward string) {
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
	fmt.Println(st.TailscaleIPs[0])
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
