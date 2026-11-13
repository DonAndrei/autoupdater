export interface OutdatedApp {
  key: string;
  id: string;
  name: string;
  current: string;
  available: string;
  source: "winget" | "msstore" | "brew" | "cask" | "mas" | string;
}

export interface ScanResult {
  apps: OutdatedApp[];
  warnings: string[];
  checkedAt: number;
}

export interface Progress {
  key: string;
  status: "running" | "done" | "failed";
  line: string | null;
  error: string | null;
}

export interface Finished {
  ok: number;
  failed: number;
}

export type RowState = "queued" | "running" | "done" | "failed";

export interface RowStatus {
  state: RowState;
  log: string[];
  error?: string;
}
