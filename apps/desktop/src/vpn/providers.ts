import { vpnProviderName } from "@/hosts/HostBadges";
import { tr } from "@/i18n";

/** Providers a VPN network can use, in the order the editor offers them. */
export const VPN_PROVIDERS = ["netbird", "tailscale"] as const;

/** The server a network uses when it names none: the provider's cloud. */
export function providerCloud(provider: string): string {
  return provider === "tailscale" ? "https://controlplane.tailscale.com" : "https://api.netbird.io";
}

/** How long a network may stay up without a connection, in seconds; null
 *  = the app's default (2 minutes), 0 = until disconnected. */
export const IDLE_CHOICES: readonly (number | null)[] = [30, null, 600, 3600, 0];

/** The editor's label for an idle choice. */
export function idleChoiceLabel(secs: number | null): string {
  switch (secs) {
    case null:
      return tr("2 minutes (default)");
    case 0:
      return tr("Until I disconnect");
    case 30:
      return tr("30 seconds");
    case 600:
      return tr("10 minutes");
    case 3600:
      return tr("1 hour");
    default:
      return secs < 120
        ? tr("{count} s", { count: secs })
        : tr("{count} min", { count: Math.round(secs / 60) });
  }
}

/** Fallback title for a network without a label. */
export function networkTitle(n: { label: string; provider: string }): string {
  return n.label || vpnProviderName(n.provider);
}
