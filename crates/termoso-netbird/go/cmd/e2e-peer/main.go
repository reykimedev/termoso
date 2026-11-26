// Command e2e-peer is the far end of the termoso-netbird end-to-end test: a
// NetBird peer that listens on :2222 inside the network and forwards each
// connection to -forward (an SSH server), or echoes when -forward is empty.
// It prints its NetBird IP on stdout once up. Not shipped in any app.
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
)

func main() {
	mgmt := flag.String("management", "", "management URL")
	forward := flag.String("forward", "", "host:port to forward to; empty = echo")
	flag.Parse()
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
