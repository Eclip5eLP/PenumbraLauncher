// Typed wrappers around Tauri commands/events. The only place that calls `invoke`/`listen`.
// Outside Tauri (plain browser during UI development) calls go to a dev-only mock backend.
import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppError, Bootstrap, Job, Library, Settings } from "./types";

export const inTauri = isTauri();

const mock = inTauri || !import.meta.env.DEV ? null : import("./browser-mock");

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (mock) return (await mock).mockInvoke(cmd, args) as Promise<T>;
  return tauriInvoke<T>(cmd, args);
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  appReady: (firstPaintMs: number) => invoke<number>("app_ready", { firstPaintMs }),
  setSettings: (patch: Partial<Settings>) => invoke<Settings>("set_settings", { patch }),
  install: (item: string, channel: string, path?: string) => invoke<Job>("install", { item, channel, path }),
  pauseJob: (id: number) => invoke<void>("pause_job", { id }),
  resumeJob: (id: number) => invoke<void>("resume_job", { id }),
  cancelJob: (id: number) => invoke<void>("cancel_job", { id }),
  prioritizeJob: (id: number) => invoke<void>("prioritize_job", { id }),
  clearFinishedJobs: () => invoke<void>("clear_finished_jobs"),
  uninstall: (item: string) => invoke<void>("uninstall", { item }),
  launch: (item: string) => invoke<void>("launch", { item }),
  openItemLink: (item: string, key?: string) => invoke<void>("open_item_link", { item, key }),
};

export interface Events {
  "jobs:changed": Job[];
  "library:changed": Library;
  "settings:changed": Settings;
  "process:changed": { item: string; running: boolean };
}

export async function on<K extends keyof Events>(
  name: K,
  cb: (payload: Events[K]) => void,
): Promise<UnlistenFn> {
  if (mock) return (await mock).mockListen(name, cb as (p: unknown) => void);
  return listen<Events[K]>(name, (e) => cb(e.payload));
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as AppError).message);
  return String(e);
}
