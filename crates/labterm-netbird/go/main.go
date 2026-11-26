// Command labterm-netbird is the embedded NetBird client of Labterm, built
// as a C shared library (`-buildmode=c-shared`) and loaded by the
// labterm-netbird crate. It runs NetBird in userspace (netstack): no TUN
// device, no VpnService and no other app. Each dial hands back one end of a
// Unix socket pair, so the Rust side sees a plain stream and the SSH session
// stays end-to-end on top of it.
package main

/*
#include <stdint.h>
#include <stdlib.h>
*/
import "C"

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"runtime/cgo"
	"strconv"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"github.com/netbirdio/netbird/client/embed"
)

// startOptions is the JSON the Rust side passes to tnb_start.
type startOptions struct {
	ManagementURL string `json:"management_url"`
	SetupKey      string `json:"setup_key"`
	// WireGuard private key of this device's peer from an earlier start;
	// tried first so the phone keeps one peer instead of registering anew.
	PrivateKey string `json:"private_key"`
	DeviceName string `json:"device_name"`
	LogLevel   string `json:"log_level"`
	TimeoutMs  int    `json:"timeout_ms"`
}

type startResult struct {
	PrivateKey string `json:"private_key"`
	// Registered is true when this start used the setup key (a new peer).
	Registered bool `json:"registered"`
}

type node struct {
	client *embed.Client
	mu     sync.Mutex
	conns  map[net.Conn]struct{}
}

func cstr(s string) *C.char { return C.CString(s) }

func newClient(o startOptions, privateKey, setupKey string) (*embed.Client, error) {
	level := o.LogLevel
	if level == "" {
		level = "warn"
	}
	return embed.New(embed.Options{
		DeviceName:    o.DeviceName,
		SetupKey:      setupKey,
		PrivateKey:    privateKey,
		ManagementURL: o.ManagementURL,
		LogOutput:     os.Stderr,
		LogLevel:      level,
		// A terminal only dials out: nobody on the network may open
		// connections to the phone, and it must never route for others.
		BlockInbound:   true,
		BlockLANAccess: true,
		// Keep everything in memory: the peer key comes from and goes back
		// to Labterm's encrypted device store.
		ConfigPath: "",
		StatePath:  "",
	})
}

func start(o startOptions) (*embed.Client, startResult, error) {
	timeout := time.Duration(o.TimeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = 30 * time.Second
	}
	try := func(privateKey, setupKey string) (*embed.Client, error) {
		c, err := newClient(o, privateKey, setupKey)
		if err != nil {
			return nil, err
		}
		ctx, cancel := context.WithTimeout(context.Background(), timeout)
		defer cancel()
		if err := c.Start(ctx); err != nil {
			return nil, err
		}
		return c, nil
	}
	if o.PrivateKey != "" {
		c, err := try(o.PrivateKey, "")
		if err == nil {
			return c, startResult{PrivateKey: o.PrivateKey}, nil
		}
		if o.SetupKey == "" {
			return nil, startResult{}, err
		}
		// The peer is gone (ephemeral key expired, removed in the
		// dashboard): register again with the setup key below.
	}
	if o.SetupKey == "" {
		return nil, startResult{}, errors.New("no setup key")
	}
	c, err := try("", o.SetupKey)
	if err != nil {
		return nil, startResult{}, err
	}
	cfg, err := c.GetConfig()
	if err != nil {
		_ = c.Stop(context.Background())
		return nil, startResult{}, err
	}
	return c, startResult{PrivateKey: cfg.PrivateKey, Registered: true}, nil
}

// tnb_start starts a node. On success it stores the handle in *handle and
// returns a JSON startResult; on failure *handle stays 0 and the returned
// string is the error. Free the string with tnb_free.
//
//export tnb_start
func tnb_start(optsJSON *C.char, handle *C.uintptr_t) *C.char {
	*handle = 0
	var o startOptions
	if err := json.Unmarshal([]byte(C.GoString(optsJSON)), &o); err != nil {
		return cstr("bad options: " + err.Error())
	}
	c, res, err := start(o)
	if err != nil {
		return cstr(err.Error())
	}
	out, _ := json.Marshal(res)
	n := &node{client: c, conns: map[net.Conn]struct{}{}}
	*handle = C.uintptr_t(cgo.NewHandle(n))
	return cstr(string(out))
}

func lookup(h C.uintptr_t) (*node, bool) {
	if h == 0 {
		return nil, false
	}
	n, ok := cgo.Handle(h).Value().(*node)
	return n, ok
}

// tnb_dial opens a TCP connection to host:port inside the NetBird network
// and returns a file descriptor (one end of a Unix socket pair) that carries
// its bytes, or -1 with the error in *errOut (free with tnb_free).
//
//export tnb_dial
func tnb_dial(h C.uintptr_t, host *C.char, port C.int, timeoutMs C.int, errOut **C.char) C.int {
	*errOut = nil
	n, ok := lookup(h)
	if !ok {
		*errOut = cstr("node is not running")
		return -1
	}
	timeout := time.Duration(timeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = 20 * time.Second
	}
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	addr := net.JoinHostPort(C.GoString(host), strconv.Itoa(int(port)))
	remote, err := n.client.Dial(ctx, "tcp", addr)
	if err != nil {
		*errOut = cstr(fmt.Sprintf("netbird: %v", err))
		return -1
	}
	fds, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM|syscall.SOCK_CLOEXEC, 0)
	if err != nil {
		remote.Close()
		*errOut = cstr(fmt.Sprintf("socketpair: %v", err))
		return -1
	}
	f := os.NewFile(uintptr(fds[0]), "netbird-conn")
	local, err := net.FileConn(f)
	f.Close()
	if err != nil {
		syscall.Close(fds[1])
		remote.Close()
		*errOut = cstr(fmt.Sprintf("fileconn: %v", err))
		return -1
	}
	n.mu.Lock()
	n.conns[remote] = struct{}{}
	n.mu.Unlock()
	go pipe(n, local, remote)
	return C.int(fds[1])
}

func pipe(n *node, local, remote net.Conn) {
	done := make(chan struct{}, 2)
	cp := func(dst, src net.Conn) {
		_, _ = io.Copy(dst, src)
		if cw, ok := dst.(interface{ CloseWrite() error }); ok {
			_ = cw.CloseWrite()
		}
		done <- struct{}{}
	}
	go cp(remote, local)
	go cp(local, remote)
	<-done
	<-done
	local.Close()
	remote.Close()
	n.mu.Lock()
	delete(n.conns, remote)
	n.mu.Unlock()
}

// tnb_stop closes every connection of the node, stops it and frees the
// handle.
//
//export tnb_stop
func tnb_stop(h C.uintptr_t) {
	n, ok := lookup(h)
	if !ok {
		return
	}
	n.mu.Lock()
	for c := range n.conns {
		c.Close()
	}
	n.mu.Unlock()
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	_ = n.client.Stop(ctx)
	cgo.Handle(h).Delete()
}

// tnb_free frees a string returned by this library.
//
//export tnb_free
func tnb_free(s *C.char) {
	C.free(unsafe.Pointer(s))
}

func main() {}
