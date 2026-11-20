// Mirrors the Rust types (crates/penumbra-core/src/catalog.rs, src-tauri/src/{settings,jobs,state}.rs).
// Keep in sync by hand until tauri-specta is introduced.

export type ItemType = "game" | "software" | "installer" | "link";
export type ItemStatus = "released" | "coming-soon";

export interface Media {
  icon?: string | null;
  logo?: string | null;
  bg?: string | null;
  bgVideo?: string | null;
  cover?: string | null;
  accent?: string | null;
}

export interface Channel {
  version?: string | null;
  size?: number | null;
  download?: number | null;
  access?: string | null;
  mode?: "release" | "branch" | "tag" | null;
  required?: boolean;
}

export interface NewsPost {
  id: string;
  category: "event" | "notice" | "info" | string;
  title: string;
  date: string;
  url?: string | null;
}

export interface Banner {
  id: string;
  image: string;
  title?: string | null;
  url?: string | null;
}

export type Source =
  | { kind: "penumbra" }
  | { kind: "github"; repo: string; stripRoot?: boolean; unpack?: boolean; keep?: string[] }
  | { kind: "url"; unpack?: boolean; keep?: string[] };

export interface Item {
  id: string;
  type: ItemType;
  group: string;
  name: string;
  tagline?: string | null;
  description?: string | null;
  status: ItemStatus;
  media: Media;
  access: string;
  locked: boolean;
  url?: string | null;
  source?: Source | null;
  channels: Record<string, Channel>;
  news: NewsPost[];
  banners: Banner[];
  links: Record<string, string>;
}

export interface Group {
  id: string;
  name: string;
}

export interface Catalog {
  format: number;
  generated: string;
  mirrors: string[];
  groups: Group[];
  items: Item[];
}

export interface Installed {
  version: string;
  channel: string;
  path: string;
  size: number;
  playtimeSec: number;
  lastPlayed?: number | null;
}

export type Library = Record<string, Installed>;

export type JobKind = "install" | "update" | "repair";
export type JobState =
  | "queued"
  | "preparing"
  | "verifying"
  | "downloading"
  | "paused"
  | "finalizing"
  | "done"
  | "error"
  | "cancelled";

export interface Job {
  id: number;
  item: string;
  kind: JobKind;
  channel: string;
  version: string;
  path: string;
  state: JobState;
  downloaded: number;
  total: number;
  speed: number;
  eta?: number | null;
  error?: string | null;
  note?: string | null;
  finishedAt?: number | null;
}

export interface Settings {
  schema: number;
  language: string;
  startWithWindows: boolean;
  startMinimized: boolean;
  closeButton: "tray" | "exit";
  onGameLaunch: "minimize" | "tray" | "stay" | "close";
  launcherChannel: "stable" | "beta";
  autoUpdateLauncher: boolean;
  notifications: boolean;
  libraries: string[];
  defaultLibrary?: string | null;
  autoUpdateGames: boolean;
  maxConcurrentJobs: number;
  connectionsPerJob: number;
  bandwidthLimit: number;
  pauseWhileRunning: boolean;
  resumeOnStart: boolean;
  verifyAfterInstall: boolean;
  createDesktopShortcut: "ask" | "always" | "never";
  backgroundVideo: boolean;
  reduceMotion: "system" | "on" | "off";
  windowEffect: "none" | "mica";
  uiScale: number;
  useGameAccent: boolean;
  railOrder: string[];
  hiddenItems: string[];
  pinnedItems: string[];
  syncSettings: boolean;
  cloudSaves: boolean;
  shareTokenWithGames: boolean;
  logLevel: string;
  apiBase: string;
  offlineMode: boolean;
  cacheLimitMb: number;
  devMode: boolean;
}

export interface CatalogStatus {
  state: "loading" | "ok" | "offline" | "error";
  message?: string | null;
}

export interface Bootstrap {
  appVersion: string;
  settings: Settings;
  catalog: Catalog;
  library: Library;
  jobs: Job[];
  running: string[];
  catalogStatus: CatalogStatus;
  suggestedLibrary: string;
  mock: boolean;
}

export interface AppError {
  code: string;
  message: string;
}
