package main

import (
	"context"
	"strings"
	"sync"
	"time"

	"github.com/netbirdio/netbird/client/embed"
	sharedgrpc "github.com/netbirdio/netbird/shared/management/grpc"
	nbnetworkmap "github.com/netbirdio/netbird/shared/management/networkmap"
	mgmProto "github.com/netbirdio/netbird/shared/management/proto"
	"golang.zx2c4.com/wireguard/wgctrl/wgtypes"
)

// Search domains come from the network map's DNS config, which the status
// doesn't carry. Decoding a components-format map runs NetBird's whole
// network map calculation, so the result is kept per client and map serial.
type searchCache struct {
	mu      sync.Mutex
	serial  uint64
	domains []string
	ok      bool
}

var searchCaches sync.Map // *embed.Client -> *searchCache

func dropSearchCache(c *embed.Client) { searchCaches.Delete(c) }

// searchDomains lists the network's search domains, most specific source
// first: the peer domain (peer names), custom zones, then the match domains
// of nameserver groups with search domains on. Right after start it waits
// briefly for the first network map, like the nameserver lookup does.
func (b nbNode) searchDomains(ctx context.Context) []string {
	for {
		if d, ok := loadSearchDomains(b.c); ok || time.Since(b.started) >= dnsSettle {
			return d
		}
		select {
		case <-ctx.Done():
			return nil
		case <-time.After(200 * time.Millisecond):
		}
	}
}

func loadSearchDomains(c *embed.Client) ([]string, bool) {
	resp, err := c.GetLatestSyncResponse()
	if err != nil || resp == nil {
		return nil, false
	}
	v, _ := searchCaches.LoadOrStore(c, &searchCache{})
	sc := v.(*searchCache)
	sc.mu.Lock()
	defer sc.mu.Unlock()
	serial := syncSerial(resp)
	if sc.ok && sc.serial == serial {
		return sc.domains, true
	}
	nm := networkMap(c, resp)
	if nm == nil {
		return nil, false
	}
	sc.serial, sc.ok = serial, true
	sc.domains = searchList(peerDomain(resp.GetPeerConfig().GetFqdn()), nm.GetDNSConfig())
	return sc.domains, true
}

func syncSerial(resp *mgmProto.SyncResponse) uint64 {
	if nm := resp.GetNetworkMap(); nm != nil {
		return nm.GetSerial()
	}
	return resp.GetNetworkMapEnvelope().GetFull().GetSerial()
}

func networkMap(c *embed.Client, resp *mgmProto.SyncResponse) *mgmProto.NetworkMap {
	if resp.GetVersion() != int32(sharedgrpc.ComponentNetworkMap) {
		return resp.GetNetworkMap()
	}
	cfg, err := c.GetConfig()
	if err != nil {
		return nil
	}
	key, err := wgtypes.ParseKey(cfg.PrivateKey)
	if err != nil {
		return nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	res, err := nbnetworkmap.EnvelopeToNetworkMap(ctx, resp.GetNetworkMapEnvelope(),
		key.PublicKey().String(), peerDomain(resp.GetPeerConfig().GetFqdn()), true)
	if err != nil {
		return nil
	}
	return res.NetworkMap
}

// searchList builds the ordered, de-duplicated search list.
func searchList(peerDomain string, cfg *mgmProto.DNSConfig) []string {
	var out []string
	seen := map[string]bool{}
	add := func(d string) {
		d = strings.Trim(strings.ToLower(d), ".")
		if d != "" && !seen[d] {
			seen[d] = true
			out = append(out, d)
		}
	}
	add(peerDomain)
	for _, z := range cfg.GetCustomZones() {
		if !z.GetSearchDomainDisabled() {
			add(z.GetDomain())
		}
	}
	for _, g := range cfg.GetNameServerGroups() {
		if g.GetSearchDomainsEnabled() {
			for _, d := range g.GetDomains() {
				add(d)
			}
		}
	}
	return out
}
