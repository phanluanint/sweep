// Typed wrappers around the Rust commands and events (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Risk = "safe" | "review" | "info";

export interface CleanupItem {
  path: string;
  name: string;
  bytes: number;
  files: number;
}

export interface GroupScan {
  id: string;
  label: string;
  description: string;
  icon: string;
  risk: Risk;
  defaultSelected: boolean;
  available: boolean;
  cleanable: boolean;
  bytes: number;
  files: number;
  items: CleanupItem[];
  skippedInUse: string[];
  errors: number;
}

export interface Skip {
  path: string;
  reason: string;
}

export interface TrashReport {
  moved: string[];
  bytesMoved: number;
  skipped: Skip[];
  failed: Skip[];
}

export interface ScanStats {
  root: string;
  bytes: number;
  files: number;
  dirs: number;
  errors: number;
  otherVolumes: number;
  elapsedMs: number;
}

export interface ScanProgress {
  files: number;
  bytes: number;
  current: string;
}

export interface DiskChild {
  path: string;
  name: string;
  bytes: number;
  files: number;
  kind: "dir" | "files";
  hasChildren: boolean;
}

export type FileKind = "video" | "archive" | "installer" | "disk-image" | "other";

export interface FileInfo {
  path: string;
  name: string;
  parent: string;
  bytes: number;
  lastUsedSecs: number;
  kind: FileKind;
}

export interface VolumeInfo {
  total: number;
  free: number;
}

export interface CpuSnapshot {
  brand: string;
  total: number;
  perCore: number[];
  performanceCores: number;
  efficiencyCores: number;
  loadAverage: [number, number, number];
  uptimeSecs: number;
}

export interface MemorySnapshot {
  total: number;
  used: number;
  app: number;
  wired: number;
  compressed: number;
  cached: number;
  free: number;
  swapTotal: number;
  swapUsed: number;
  pressurePct: number;
  pressureLevel: "normal" | "warning" | "critical";
}

export interface Fan {
  label: string;
  rpm: number;
  min: number;
  max: number;
}

export interface ProcInfo {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
}

export interface HealthSnapshot {
  model: string;
  cpu: CpuSnapshot;
  cpuTempC: number | null;
  memory: MemorySnapshot | null;
  fans: Fan[] | null;
  topCpu: ProcInfo[];
  topMemory: ProcInfo[];
}

export const api = {
  homeDir: () => invoke<string>("home_dir"),
  volumeInfo: () => invoke<VolumeInfo>("volume_info"),

  cleanupScan: () => invoke<GroupScan[]>("cleanup_scan"),
  cleanupClean: (paths: string[]) => invoke<TrashReport>("cleanup_clean", { paths }),

  diskScan: (root?: string) => invoke<ScanStats>("disk_scan", { root: root ?? null }),
  diskCancel: () => invoke<void>("disk_cancel"),
  diskChildren: (path: string) => invoke<[DiskChild, DiskChild[]]>("disk_children", { path }),
  diskLargest: () => invoke<FileInfo[]>("disk_largest"),
  diskTrash: (paths: string[]) => invoke<TrashReport>("disk_trash", { paths }),
  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),

  setHealthInterval: (ms: number) => invoke<void>("set_health_interval", { ms }),
};

export const events = {
  diskProgress: (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
    listen<ScanProgress>("disk://progress", (e) => cb(e.payload)),
  healthTick: (cb: (s: HealthSnapshot) => void): Promise<UnlistenFn> =>
    listen<HealthSnapshot>("health://tick", (e) => cb(e.payload)),
};
