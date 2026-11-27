import { describe, expect, it } from "vitest";
import { IDLE_CHOICES, idleChoiceLabel } from "./providers";
import { formatCountdown } from "./VpnStatusStrip";

describe("VPN idle time", () => {
  it("offers the default and 'until disconnected'", () => {
    expect(IDLE_CHOICES).toContain(null);
    expect(IDLE_CHOICES).toContain(0);
    expect(idleChoiceLabel(null)).toBe("2 minutes (default)");
    expect(idleChoiceLabel(0)).toBe("Until I disconnect");
    expect(idleChoiceLabel(300)).toBe("5 min");
  });

  it("formats the countdown", () => {
    expect(formatCountdown(0)).toBe("0:00");
    expect(formatCountdown(65)).toBe("1:05");
    expect(formatCountdown(3600)).toBe("60:00");
  });
});
