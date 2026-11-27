import { describe, expect, it } from "vitest";
import { tr, trn, trx } from "./index";

describe("tr / trn / trx", () => {
  it("interpolates", () => {
    expect(tr("Saved to {path}", { path: "/tmp/x" })).toBe("Saved to /tmp/x");
    expect(tr("not a key {n}", { n: 1 })).toBe("not a key 1");
    expect(tr("left {alone}")).toBe("left {alone}");
  });

  it("picks the singular only for one", () => {
    expect(trn(1, "{count} host", "{count} hosts")).toBe("1 host");
    expect(trn(0, "{count} host", "{count} hosts")).toBe("0 hosts");
    expect(trn(2, "{count} host", "{count} hosts")).toBe("2 hosts");
    expect(trn(21, "{count} host in {vault}", "{count} hosts in {vault}", { vault: "Lab" })).toBe(
      "21 hosts in Lab",
    );
  });

  it("splits rich placeholders into renderable pieces", () => {
    const parts = trx("Saved to {path}", { path: "X" });
    expect(parts).toHaveLength(2);
    expect(parts[0]).toBe("Saved to ");
    expect(trx("{a} and {b}", { a: 1 })).toEqual([expect.anything(), " and ", "{b}"]);
  });
});
