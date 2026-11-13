import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Finished, Progress, ScanResult } from "./types";

export const scan = () => invoke<ScanResult>("scan");

export const updateApps = (keys: string[]) => invoke<void>("update_apps", { keys });

/** Subscribes to backend events; returns one function that removes all listeners. */
export async function subscribe(handlers: {
  onScanStarted: () => void;
  onScanResult: (r: ScanResult) => void;
  onProgress: (p: Progress) => void;
  onFinished: (f: Finished) => void;
}): Promise<UnlistenFn> {
  const offs = await Promise.all([
    listen("scan-started", () => handlers.onScanStarted()),
    listen<ScanResult>("scan-result", (e) => handlers.onScanResult(e.payload)),
    listen<Progress>("update-progress", (e) => handlers.onProgress(e.payload)),
    listen<Finished>("update-finished", (e) => handlers.onFinished(e.payload)),
  ]);
  return () => offs.forEach((off) => off());
}
