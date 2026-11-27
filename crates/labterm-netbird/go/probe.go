package main

/*
#include <stdint.h>
*/
import "C"

import (
	"bufio"
	"context"
	"encoding/json"
	"net"
	"net/netip"
	"strconv"
	"strings"
	"time"
)

// probeOut is the connection test's view of one dial: how the name
// resolved, where the connection went and what answered. It never sends
// anything to the host; the SSH banner is what the server says first.
type probeOut struct {
	dialTrace
	// Remote is the address the connection reached.
	Remote    string `json:"remote,omitempty"`
	ConnectMs int64  `json:"connect_ms"`
	// Banner is the server's identification line ("SSH-2.0-OpenSSH_9.6").
	Banner string `json:"banner,omitempty"`
	Error  string `json:"error,omitempty"`
}

func probeHost(n *node, host string, port int, timeout time.Duration) probeOut {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	addr := net.JoinHostPort(host, strconv.Itoa(port))
	var out probeOut
	started := time.Now()
	var conn net.Conn
	var err error
	switch b := n.client.(type) {
	case nbNode:
		conn, err = nbDial(ctx, b, "tcp", addr, &out.dialTrace)
	default:
		out.dialTrace = dialTrace{Name: host, Via: "tailscale"}
		if _, perr := netip.ParseAddr(host); perr == nil {
			out.Via = "address"
		}
		conn, err = n.client.Dial(ctx, "tcp", addr)
	}
	out.ConnectMs = time.Since(started).Milliseconds()
	if err != nil {
		out.Error = err.Error()
		return out
	}
	defer conn.Close()
	if ta, ok := conn.RemoteAddr().(*net.TCPAddr); ok {
		out.Remote = ta.String()
	} else if conn.RemoteAddr() != nil {
		out.Remote = conn.RemoteAddr().String()
	}
	_ = conn.SetReadDeadline(time.Now().Add(5 * time.Second))
	line, err := bufio.NewReader(conn).ReadString('\n')
	if err != nil && line == "" {
		out.Error = "connected, but no SSH banner: " + err.Error()
		return out
	}
	out.Banner = strings.TrimSpace(line)
	if len(out.Banner) > 255 {
		out.Banner = out.Banner[:255]
	}
	return out
}

// tnb_probe runs the connection test's dial to host:port and returns a
// JSON probeOut. Free with tnb_free.
//
//export tnb_probe
func tnb_probe(h C.uintptr_t, host *C.char, port C.int, timeoutMs C.int) *C.char {
	n, ok := lookup(h)
	if !ok {
		out, _ := json.Marshal(probeOut{Error: "node is not running"})
		return cstr(string(out))
	}
	timeout := time.Duration(timeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = 20 * time.Second
	}
	out, _ := json.Marshal(probeHost(n, C.GoString(host), int(port), timeout))
	return cstr(string(out))
}
