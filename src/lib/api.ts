import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CaptureStatus,
  DispatchResult,
  Endpoint,
  Job,
  LogEntry,
  Settings,
} from "./types";

export const api = {
  listEndpoints: () => invoke<Endpoint[]>("list_endpoints"),
  saveEndpoint: (endpoint: Endpoint) => invoke<Endpoint>("save_endpoint", { endpoint }),
  deleteEndpoint: (id: string) => invoke<void>("delete_endpoint", { id }),
  testEndpoint: (id: string) => invoke<DispatchResult>("test_endpoint", { id }),

  listJobs: () => invoke<Job[]>("list_jobs"),
  dispatchJob: (jobId: string, endpointId: string, variables?: Record<string, string>) =>
    invoke<DispatchResult>("dispatch_job", { jobId, endpointId, variables }),
  cancelJob: (jobId: string) => invoke<void>("cancel_job", { jobId }),

  getLogs: (afterSeq?: number) => invoke<LogEntry[]>("get_logs", { afterSeq }),
  clearLogs: () => invoke<void>("clear_logs"),
  exportLogs: () => invoke<void>("export_logs"),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  getCaptureStatus: () => invoke<CaptureStatus>("get_capture_status"),
  clearSpool: () => invoke<void>("clear_spool"),
  showMainWindow: () => invoke<void>("show_main_window"),
};

export const events = {
  onJobReceived: (cb: (job: Job) => void): Promise<UnlistenFn> =>
    listen<Job>("job:received", (e) => cb(e.payload)),
  onDispatchResult: (cb: (r: DispatchResult) => void): Promise<UnlistenFn> =>
    listen<DispatchResult>("dispatch:result", (e) => cb(e.payload)),
  onLogAppended: (cb: (entry: LogEntry) => void): Promise<UnlistenFn> =>
    listen<LogEntry>("log:appended", (e) => cb(e.payload)),
  onCaptureStatus: (cb: (s: CaptureStatus) => void): Promise<UnlistenFn> =>
    listen<CaptureStatus>("capture:status", (e) => cb(e.payload)),
  onEndpointsChanged: (cb: () => void): Promise<UnlistenFn> =>
    listen("endpoints:changed", () => cb()),
};

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function fmtTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString();
  } catch {
    return iso;
  }
}
