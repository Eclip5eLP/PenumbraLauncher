// Dev-only stand-in for the Rust backend when the UI runs in a normal browser (`npm run dev`, no Tauri).
// Lets the UI be developed and screenshotted in any browser. Never bundled into the app: api.ts imports it
// dynamically only when `isTauri()` is false.
import catalog from "./dev/mock-catalog.json";
import type { Bootstrap, Catalog, Job, Library, Settings } from "./types";

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((l) => l(payload));

const settings: Settings = {
  schema: 1,
  language: "en",
  startWithWindows: false,
  startMinimized: false,
  closeButton: "tray",
  onGameLaunch: "stay",
  launcherChannel: "stable",
  autoUpdateLauncher: true,
  notifications: true,
  libraries: [],
  defaultLibrary: null,
  autoUpdateGames: true,
  maxConcurrentJobs: 1,
  connectionsPerJob: 8,
  bandwidthLimit: 0,
  pauseWhileRunning: true,
  resumeOnStart: true,
  verifyAfterInstall: false,
  createDesktopShortcut: "ask",
  backgroundVideo: true,
  reduceMotion: "system",
  windowEffect: "none",
  uiScale: 1,
  useGameAccent: true,
  railOrder: [],
  hiddenItems: [],
  pinnedItems: [],
  syncSettings: true,
  cloudSaves: true,
  shareTokenWithGames: false,
  logLevel: "info",
  apiBase: "https://eclip5e.com/penumbra/",
  offlineMode: false,
  cacheLimitMb: 1024,
  devMode: true,
  updatedAt: 0,
};
const cat = catalog as unknown as Catalog;
for (const i of cat.items) {
  i.status ??= "released";
  i.locked ??= false;
  i.news ??= [];
  i.banners ??= [];
  i.links ??= {};
  i.channels ??= {};
}
const library: Library = {
  pgbot: {
    version: "v0.9.2",
    channel: "stable",
    path: "D:\\Penumbra Games\\Pirate Galaxy AFK Bot",
    size: 40_100_000,
    playtimeSec: 12300,
    lastPlayed: 1763500000,
  },
};
let jobs: Job[] = [];
let running: string[] = [];
let nextId = 0;
let t = 0;

setInterval(() => {
  t++;
  let changed = false;
  const active = jobs.filter((j) => ["preparing", "downloading", "finalizing"].includes(j.state)).length;
  const q = jobs.find((j) => j.state === "queued");
  if (q && active < settings.maxConcurrentJobs) q.state = "preparing";
  for (const j of jobs) {
    if (j.state === "preparing") ((j.state = "downloading"), (changed = true));
    else if (j.state === "downloading") {
      const bps = 55e6 * (0.85 + 0.15 * Math.sin(t / 7));
      j.downloaded = Math.min(j.total, j.downloaded + bps / 10);
      j.speed = bps;
      j.eta = (j.total - j.downloaded) / bps;
      if (j.downloaded >= j.total) ((j.state = "finalizing"), (j.speed = 0));
      changed = true;
    } else if (j.state === "finalizing" && t % 8 === 0) {
      j.state = "done";
      j.finishedAt = Date.now() / 1000;
      library[j.item] = {
        version: j.version,
        channel: j.channel,
        path: j.path,
        size: j.total,
        playtimeSec: library[j.item]?.playtimeSec ?? 0,
      };
      emit("library:changed", { ...library });
      changed = true;
    }
  }
  if (changed)
    emit(
      "jobs:changed",
      jobs.map((j) => ({ ...j })),
    );
}, 100);

const fail = (message: string) => Promise.reject({ code: "invalid_state", message });

export const mockInvoke = async (cmd: string, args: Record<string, unknown> = {}): Promise<unknown> => {
  switch (cmd) {
    case "get_bootstrap":
      return {
        appVersion: "0.1.0-browser",
        settings,
        catalog: cat,
        library,
        jobs,
        running,
        catalogStatus: { state: "ok" },
        auth: { signedIn: false },
        updater: { state: "idle" },
        playtimeOther: {},
        suggestedLibrary: "D:\Penumbra Games",
        mock: true,
      } satisfies Bootstrap;
    case "app_ready":
      return Math.round(performance.now());
    case "set_settings":
      Object.assign(settings, args.patch);
      return { ...settings };
    case "install": {
      const item = cat.items.find((i) => i.id === args.item)!;
      const ch = item.channels[args.channel as string];
      const inst = library[item.id];
      const job: Job = {
        id: ++nextId,
        item: item.id,
        kind: inst ? "update" : "install",
        channel: args.channel as string,
        version: ch.version!,
        path: (args.path as string) ?? inst?.path ?? "",
        state: "queued",
        downloaded: 0,
        total: inst ? ch.download! / 3 : ch.download!,
        speed: 0,
        eta: null,
      };
      jobs.push(job);
      return job;
    }
    case "pause_job":
    case "resume_job":
    case "cancel_job": {
      const j = jobs.find((x) => x.id === args.id);
      if (!j) return fail("unknown job");
      j.state = cmd === "pause_job" ? "paused" : cmd === "resume_job" ? "queued" : "cancelled";
      j.speed = 0;
      emit(
        "jobs:changed",
        jobs.map((x) => ({ ...x })),
      );
      return;
    }
    case "prioritize_job":
      jobs = [...jobs.filter((j) => j.id === args.id), ...jobs.filter((j) => j.id !== args.id)];
      return;
    case "clear_finished_jobs":
      jobs = jobs.filter((j) => !["done", "error", "cancelled"].includes(j.state));
      emit("jobs:changed", jobs);
      return;
    case "uninstall":
      delete library[args.item as string];
      emit("library:changed", { ...library });
      return;
    case "launch": {
      const item = args.item as string;
      running = [...running, item];
      emit("process:changed", { item, running: true });
      setTimeout(() => {
        running = running.filter((i) => i !== item);
        emit("process:changed", { item, running: false });
      }, 8000);
      return;
    }
    case "refresh_catalog":
    case "open_folder":
    case "locate_install":
      return;
    case "pick_folder":
      return null;
    case "disk_space":
      return 512e9;
    case "verify":
    case "retry_job":
      return fail("mock: not simulated in the browser");
    case "open_item_link":
      return;
  }
  return fail(`mock: unknown command ${cmd}`);
};

export const mockListen = (name: string, cb: Listener) => {
  if (!listeners.has(name)) listeners.set(name, new Set());
  listeners.get(name)!.add(cb);
  return Promise.resolve(() => listeners.get(name)!.delete(cb));
};
