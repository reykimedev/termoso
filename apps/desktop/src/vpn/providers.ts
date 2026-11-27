import { vpnProviderName } from "@/hosts/HostBadges";

/** Providers a VPN network can use, in the order the editor offers them. */
export const VPN_PROVIDERS = ["netbird", "tailscale"] as const;

/** The server a network uses when it names none: the provider's cloud. */
export function providerCloud(provider: string): string {
  return provider === "tailscale" ? "https://controlplane.tailscale.com" : "https://api.netbird.io";
}

/** Fallback title for a network without a label. */
export function networkTitle(n: { label: string; provider: string }): string {
  return n.label || vpnProviderName(n.provider);
}
