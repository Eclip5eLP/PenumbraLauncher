// Global UI state (Svelte 5 runes). Data from Rust is kept as $state.raw: replaced wholesale on events,
// never deep-proxied, so 10 Hz progress updates stay cheap.
import { api, errorMessage, on } from "./api";
import { setLanguage, t } from "./i18n";
import type {
  AuthState,
  Bootstrap,
  Catalog,
  CatalogStatus,
  Item,
  Job,
  Library,
  Settings,
  UpdaterState,
} from "./types";

export type Route =
  | { page: "item"; id: string }
  | { page: "library" }
  | { page: "downloads" }
  | { page: "settings"; section?: string };

export type Dialog =
  { kind: "install"; item: string } | { kind: "uninstall"; item: string } | { kind: "signin" };

export interface Toast {
  id: number;
  kind: "ok" | "err" | "info";
  text: string;
}

export type Action =
  | "open"
  | "comingSoon"
  | "locked"
  | "unavailable"
  | "install"
  | "update"
  | "play"
  | "running"
  | "queued"
  | "preparing"
  | "verifying"
  | "downloading"
  | "paused"
  | "finalizing";

const LAST_ITEM_KEY = "pl.lastItem";

function readLocal(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function writeLocal(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* storage unavailable: just a convenience */
  }
}

class AppStore {
  appVersion = "";
  mock = false;
  startupMs = $state(0);
  settings = $state.raw<Settings>(undefined as unknown as Settings);
  catalog = $state.raw<Catalog>(undefined as unknown as Catalog);
  library = $state.raw<Library>({});
  jobs = $state.raw<Job[]>([]);
  running = $state.raw<string[]>([]);
  catalogStatus = $state.raw<CatalogStatus>({ state: "loading" });
  suggestedLibrary = "";
  auth = $state.raw<AuthState>({ signedIn: false });
  updater = $state.raw<UpdaterState>({ state: "idle" });
  route = $state<Route>({ page: "library" });
  channelPref = $state<Record<string, string>>({});
  dialog = $state<Dialog | null>(null);
  toasts = $state<Toast[]>([]);
  private toastId = 0;

  init(b: Bootstrap) {
    this.appVersion = b.appVersion;
    this.mock = b.mock;
    this.settings = b.settings;
    this.catalog = b.catalog;
    this.library = b.library;
    this.jobs = b.jobs;
    this.running = b.running;
    this.catalogStatus = b.catalogStatus;
    this.suggestedLibrary = b.suggestedLibrary;
    this.auth = b.auth;
    this.updater = b.updater;
    setLanguage(b.settings.language);
    this.applyMotion();
    this.pickStartRoute();
  }

  /** Last viewed item if it still exists, else the first item, else the library. */
  private pickStartRoute() {
    const last = readLocal(LAST_ITEM_KEY);
    const first = this.visibleItems[0]?.id;
    const id = last && this.item(last) ? last : first;
    this.route = id ? { page: "item", id } : { page: "library" };
  }

  async listen() {
    await Promise.all([
      on("jobs:changed", (jobs) => {
        for (const j of jobs) {
          const prev = this.jobs.find((p) => p.id === j.id);
          if (!prev || prev.state === j.state) continue;
          const name = this.item(j.item)?.name ?? j.item;
          if (j.state === "done") {
            const key =
              j.kind === "repair"
                ? "toast.repaired"
                : j.kind === "update"
                  ? "toast.updated"
                  : "toast.installed";
            this.toast("ok", j.note ? `${name}: ${j.note}` : t(key, { name }), 5000);
          } else if (j.state === "error") {
            this.toast("err", `${name}: ${j.error ?? t("toast.error")}`, 8000);
          }
        }
        this.jobs = jobs;
      }),
      on("library:changed", (lib) => (this.library = lib)),
      on("catalog:changed", (c) => {
        const wasEmpty = this.catalog.items.length === 0;
        this.catalog = c;
        const r = this.route;
        if ((r.page === "item" && !this.item(r.id)) || (wasEmpty && r.page === "library"))
          this.pickStartRoute();
      }),
      on("catalog:status", (s) => (this.catalogStatus = s)),
      on("updater:state", (u) => (this.updater = u)),
      on("auth:changed", (a) => {
        const was = this.auth.signedIn;
        this.auth = a;
        if (a.signedIn && !was) {
          if (this.dialog?.kind === "signin") this.dialog = null;
          this.toast("ok", t("account.welcome", { name: a.user?.username ?? "" }));
        } else if (!a.signedIn && was) {
          this.toast("info", t("account.signedOut"));
        }
      }),
      on("settings:changed", (s) => {
        this.settings = s;
        setLanguage(s.language);
        this.applyMotion();
      }),
      on("process:changed", ({ item, running }) => {
        this.running = running
          ? [...new Set([...this.running, item])]
          : this.running.filter((i) => i !== item);
      }),
    ]);
  }

  // ---- derived helpers -------------------------------------------------------------------------------
  get visibleItems(): Item[] {
    const hidden = new Set(this.settings.hiddenItems);
    return this.catalog.items.filter((i) => !hidden.has(i.id));
  }

  item(id: string): Item | undefined {
    return this.catalog.items.find((i) => i.id === id);
  }

  activeJob(id: string): Job | undefined {
    return this.jobs.find((j) => j.item === id && !["done", "error", "cancelled"].includes(j.state));
  }

  get activeJobCount(): number {
    return this.jobs.filter((j) => !["done", "error", "cancelled"].includes(j.state)).length;
  }

  /** Channel shown for an item: installed channel > user pick > stable > first. */
  channelOf(item: Item): string {
    const picked = this.channelPref[item.id];
    if (picked && item.channels[picked]) return picked;
    const inst = this.library[item.id];
    if (inst && item.channels[inst.channel]) return inst.channel;
    if (item.channels.stable) return "stable";
    return Object.keys(item.channels)[0] ?? "stable";
  }

  action(item: Item): Action {
    if (item.type === "link") return "open";
    if (item.status === "coming-soon") return "comingSoon";
    if (item.locked || (item.access !== "public" && !this.auth.signedIn)) return "locked";
    const job = this.activeJob(item.id);
    if (job) return job.state as Action;
    if (this.running.includes(item.id)) return "running";
    const inst = this.library[item.id];
    const ch = item.channels[this.channelOf(item)];
    if (!inst) return ch?.version ? "install" : "unavailable";
    if (ch?.version && (ch.version !== inst.version || this.channelOf(item) !== inst.channel))
      return "update";
    return "play";
  }

  // ---- navigation ------------------------------------------------------------------------------------
  go(route: Route) {
    this.route = route;
    if (route.page === "item") writeLocal(LAST_ITEM_KEY, route.id);
  }

  // ---- commands --------------------------------------------------------------------------------------
  async run<T>(p: Promise<T>): Promise<T | undefined> {
    try {
      return await p;
    } catch (e) {
      this.toast("err", errorMessage(e));
      return undefined;
    }
  }

  async primary(item: Item) {
    const a = this.action(item);
    const job = this.activeJob(item.id);
    switch (a) {
      case "open":
        return this.run(api.openItemLink(item.id));
      case "install":
        this.dialog = { kind: "install", item: item.id };
        return;
      case "update":
        return this.run(api.install(item.id, this.channelOf(item)));
      case "play":
        return this.run(api.launch(item.id));
      case "downloading":
      case "preparing":
      case "verifying":
      case "queued":
        return job && this.run(api.pauseJob(job.id));
      case "locked":
        this.dialog = { kind: "signin" };
        return;
      case "paused":
        return job && this.run(api.resumeJob(job.id));
    }
  }

  async setSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
    const next = await this.run(api.setSettings({ [key]: value } as Partial<Settings>));
    if (next) this.settings = next;
  }

  toast(kind: Toast["kind"], text: string, ms = 4000) {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, kind, text }].slice(-4);
    setTimeout(() => (this.toasts = this.toasts.filter((x) => x.id !== id)), ms);
  }

  private applyMotion() {
    const m = this.settings.reduceMotion;
    const reduce =
      m === "on" || (m === "system" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    document.documentElement.toggleAttribute("data-reduce-motion", reduce);
    document.documentElement.style.setProperty("--ui-scale", String(this.settings.uiScale));
  }
}

export const app = new AppStore();
