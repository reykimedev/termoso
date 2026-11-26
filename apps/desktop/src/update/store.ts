// Updater UI state. Rust owns the check/verify/install pipeline; this only
// mirrors its events so Settings and the shell banner render the same thing.

import * as ipc from "@/ipc/commands";
import type { UpdateInfo } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { createStore, useStore } from "@/lib/store";
import { useAppInfo } from "@/ipc/hooks";

/**
 * Rust decides (update.rs): updates are on only in a build that carries
 * Labterm's own signing key. Off while unknown, and in Labterm Web.
 */
export const useUpdatesEnabled = () => useAppInfo().data?.updatesEnabled === true;

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate"; checkedAt: number }
  | { kind: "available"; info: UpdateInfo }
  | {
      kind: "downloading";
      info: UpdateInfo;
      /** Download, then signature check, then the platform installer. */
      stage: "download" | "verify" | "install";
      downloaded: number;
      total: number | null;
      /** Smoothed download speed in bytes per second, null until measured. */
      speed: number | null;
      /** Last progress sample, for the speed. */
      sampledAt: number;
      sampledBytes: number;
    }
  | { kind: "installed"; version: string }
  | { kind: "error"; message: string };

export interface UpdateState {
  phase: UpdatePhase;
  /** Set when the startup check found a release and the user has not dismissed it. */
  banner: boolean;
}

export const updateStore = createStore<UpdateState>({ phase: { kind: "idle" }, banner: false });

export const useUpdate = <S>(selector: (s: UpdateState) => S) => useStore(updateStore, selector);

const setPhase = (phase: UpdatePhase) => updateStore.set((s) => ({ ...s, phase }));

type Downloading = Extract<UpdatePhase, { kind: "downloading" }>;

/** Weight of the newest sample in the speed average: steady, but follows real changes. */
const SPEED_SMOOTHING = 0.3;

/** Fold one progress event into the phase, updating the smoothed speed. */
export function withProgress(
  p: Downloading,
  downloaded: number,
  total: number | null,
  now: number,
): Downloading {
  const dt = (now - p.sampledAt) / 1000;
  // Samples closer than 0.5 s are too noisy; keep accumulating until one is far enough.
  if (dt < 0.5) return { ...p, downloaded, total };
  const instant = (downloaded - p.sampledBytes) / dt;
  const speed =
    p.speed === null ? instant : SPEED_SMOOTHING * instant + (1 - SPEED_SMOOTHING) * p.speed;
  return { ...p, downloaded, total, speed, sampledAt: now, sampledBytes: downloaded };
}

/** Seconds left at the current speed, or null when unknown. */
export function secondsLeft(p: Downloading): number | null {
  if (!p.total || !p.speed || p.speed <= 0) return null;
  return Math.max(0, Math.ceil((p.total - p.downloaded) / p.speed));
}

export async function checkForUpdates(): Promise<void> {
  const cur = updateStore.get().phase;
  if (cur.kind === "checking" || cur.kind === "downloading") return;
  setPhase({ kind: "checking" });
  try {
    const info = await ipc.updateCheck();
    setPhase(info ? { kind: "available", info } : { kind: "upToDate", checkedAt: Date.now() });
  } catch (e) {
    setPhase({ kind: "error", message: errorMessage(e) });
  }
}

export async function installUpdate(): Promise<void> {
  const cur = updateStore.get().phase;
  if (cur.kind !== "available") return;
  setPhase({
    kind: "downloading",
    info: cur.info,
    stage: "download",
    downloaded: 0,
    total: null,
    speed: null,
    sampledAt: Date.now(),
    sampledBytes: 0,
  });
  try {
    const info = await ipc.updateInstall();
    updateStore.set({ phase: { kind: "installed", version: info.version }, banner: false });
  } catch (e) {
    setPhase({ kind: "error", message: errorMessage(e) });
  }
}

export const restartToUpdate = () => ipc.updateRestart();

export const dismissBanner = () => updateStore.set((s) => ({ ...s, banner: false }));

let started = false;
export function startUpdateEvents() {
  if (started) return;
  started = true;
  void ipc.onUpdateEvent((ev) => {
    switch (ev.type) {
      case "available":
        updateStore.set({ phase: { kind: "available", info: ev.info }, banner: true });
        break;
      case "progress":
        updateStore.set((s) =>
          s.phase.kind === "downloading"
            ? { ...s, phase: withProgress(s.phase, ev.downloaded, ev.total, Date.now()) }
            : s,
        );
        break;
      case "verifying":
      case "installing": {
        const stage = ev.type === "verifying" ? "verify" : "install";
        updateStore.set((s) =>
          s.phase.kind === "downloading" ? { ...s, phase: { ...s.phase, stage } } : s,
        );
        break;
      }
      case "installed":
        updateStore.set({ phase: { kind: "installed", version: ev.version }, banner: false });
        break;
      case "failed":
        setPhase({ kind: "error", message: ev.message });
        break;
    }
  });
}
