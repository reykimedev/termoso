import { describe, expect, it } from "vitest";

import { canonicalThemeId, LABTERM_DARK, LABTERM_LIGHT, resolveTerminalTheme } from "./themes";

// WCAG contrast ratio of two #rrggbb colours.
function contrast(a: string, b: string): number {
  const lum = (hex: string) =>
    [0.2126, 0.7152, 0.0722].reduce((sum, weight, i) => {
      const c = parseInt(hex.slice(1 + 2 * i, 3 + 2 * i), 16) / 255;
      return sum + weight * (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
    }, 0);
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

describe("Labterm terminal schemes", () => {
  it("opens ids saved before the rename as the Labterm pair", () => {
    expect(canonicalThemeId("termoso-dark")).toBe(LABTERM_DARK); // rename:keep
    expect(canonicalThemeId("termoso-light")).toBe(LABTERM_LIGHT); // rename:keep
    expect(resolveTerminalTheme("termoso-light", "dark").id).toBe(LABTERM_LIGHT); // rename:keep
    expect(canonicalThemeId("nord-dark")).toBe("nord-dark");
  });

  it("uses the lantern orange cursor", () => {
    expect(resolveTerminalTheme(LABTERM_DARK, "dark").cursor).toBe("#F59A23");
    expect(resolveTerminalTheme(undefined, "light").name).toBe("Labterm Light");
  });

  it("keeps Labterm Dark readable: every colour but black at 4.5:1 or more", () => {
    const dark = resolveTerminalTheme(LABTERM_DARK, "dark");
    for (const c of [dark.foreground, dark.cursor, ...dark.ansi.slice(1)]) {
      expect(contrast(c, dark.background), c).toBeGreaterThanOrEqual(4.5);
    }
  });
});
