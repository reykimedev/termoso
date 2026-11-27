import { describe, expect, it } from "vitest";
import type { HostCard } from "@/ipc/types";
import { hostFacts, vpnProviderName } from "./HostBadges";

const card = (over: Partial<HostCard>) => ({ useMosh: false, ...over }) as HostCard;
const none = {
  auth: null,
  overlay: false,
  overlayPaused: false,
  jump: false,
  gateway: false,
  proxy: false,
  hostKeyTrusted: null,
};

describe("hostFacts", () => {
  it("is empty for a card without badges (older backend)", () => {
    expect(hostFacts(card({}))).toEqual([]);
  });

  it("lists auth, overlay, untrusted key and route in the Android order", () => {
    const facts = hostFacts(
      card({
        useMosh: true,
        badges: {
          ...none,
          auth: "key",
          overlay: true,
          hostKeyTrusted: false,
          jump: true,
          proxy: true,
        },
      }),
    );
    expect(facts.map((f) => f.key)).toEqual([
      "auth",
      "overlay",
      "hostKey",
      "jump",
      "proxy",
      "mosh",
    ]);
    expect(facts.find((f) => f.key === "hostKey")?.warn).toBe(true);
  });

  it("shows a paused overlay instead of an active one", () => {
    const facts = hostFacts(card({ badges: { ...none, overlay: true, overlayPaused: true } }));
    expect(facts).toHaveLength(1);
    expect(facts[0]?.warn).toBe(true);
  });
});

describe("vpnProviderName", () => {
  it("names known providers and passes others through", () => {
    expect(vpnProviderName("netbird")).toBe("NetBird");
    expect(vpnProviderName("wireguard")).toBe("wireguard");
  });
});
