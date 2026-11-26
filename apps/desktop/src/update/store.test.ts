import { describe, expect, it } from "vitest";
import { secondsLeft, withProgress } from "./store";

const start = {
  kind: "downloading" as const,
  info: {
    currentVersion: "0.12.1",
    version: "0.12.2",
    notes: null,
    publishedAt: null,
    downloadUrl: "https://example.com/Labterm.AppImage",
    target: "linux-x86_64",
  },
  stage: "download" as const,
  downloaded: 0,
  total: 10_000_000,
  speed: null,
  sampledAt: 0,
  sampledBytes: 0,
};

describe("update download progress", () => {
  it("measures speed once a sample is far enough apart", () => {
    const early = withProgress(start, 100_000, 10_000_000, 200);
    expect(early.speed).toBeNull();
    expect(early.downloaded).toBe(100_000);
    const p = withProgress(early, 2_000_000, 10_000_000, 1000);
    expect(p.speed).toBe(2_000_000);
    expect(secondsLeft(p)).toBe(4);
  });

  it("smooths later samples", () => {
    const a = withProgress(start, 2_000_000, 10_000_000, 1000);
    const b = withProgress(a, 3_000_000, 10_000_000, 2000);
    // 0.3 × 1 MB/s + 0.7 × 2 MB/s
    expect(b.speed).toBeCloseTo(1_700_000);
  });

  it("has no estimate without a size", () => {
    const p = withProgress({ ...start, total: null }, 2_000_000, null, 1000);
    expect(secondsLeft(p)).toBeNull();
  });
});
