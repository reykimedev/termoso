import { describe, expect, it } from "vitest";

import { canonicalThemeId, LABTERM_DARK, LABTERM_LIGHT, resolveTerminalTheme } from "./themes";

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
});
