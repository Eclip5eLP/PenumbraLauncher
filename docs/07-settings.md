# 07 — Settings

Stored in `%APPDATA%\Penumbra Launcher\settings.json` (`"schema": n` + migrations in `settings.rs`).
Column **Sync** = included in cross-device settings sync (machine-specific things never sync).

## General
| Key | Default | Sync | Notes |
|-----|---------|------|-------|
| `language` | system (en/de), fallback en | ✓ | |
| `startWithWindows` | false | | registry Run key (per user) |
| `startMinimized` | false | | only applies to autostart |
| `closeButton` | `"tray"` | ✓ | `tray` \| `exit` |
| `onGameLaunch` | `"minimize"` | ✓ | `minimize` \| `tray` \| `stay` \| `close` |
| `launcherChannel` | `"stable"` | | `stable` \| `beta` |
| `autoUpdateLauncher` | true | | download in background, apply on next start/restart |
| `notifications` | true | ✓ | Windows toasts for finished downloads |

## Downloads
| Key | Default | Sync | Notes |
|-----|---------|------|-------|
| `libraries` | `[]` → first-run asks; suggestion `<largest drive>:\Penumbra Games` | | list of folders |
| `defaultLibrary` | first library | | |
| `autoUpdateGames` | true | ✓ | per-item override |
| `maxConcurrentJobs` | 1 | ✓ | 1–3 |
| `connectionsPerJob` | 8 | ✓ | 1–32 |
| `bandwidthLimit` | 0 (unlimited) | | KB/s |
| `pauseWhileRunning` | true | ✓ | pause downloads while a game runs |
| `resumeOnStart` | true | ✓ | |
| `verifyAfterInstall` | false | ✓ | full hash pass after finishing (chunks are verified anyway) |
| `createDesktopShortcut` | ask | ✓ | `ask` \| `always` \| `never` |

## Appearance
| Key | Default | Sync | Notes |
|-----|---------|------|-------|
| `backgroundVideo` | true | ✓ | off = static image (performance) |
| `reduceMotion` | system | ✓ | |
| `windowEffect` | `"none"` | | `none` \| `mica` |
| `uiScale` | 1.0 | | 0.8–1.25 |
| `useGameAccent` | true | ✓ | tint accent per item |
| `railOrder`, `hiddenItems`, `pinnedItems` | [] | ✓ | |

## Account & cloud
| Key | Default | Sync | Notes |
|-----|---------|------|-------|
| `syncSettings` | true | — | when signed in |
| `cloudSaves` | true | ✓ | per-item override |
| `shareTokenWithGames` | false | ✓ | per-item consent, only for items with `passToken` |

## Advanced
| Key | Default | Notes |
|-----|---------|-------|
| `logLevel` | `info` | `debug` adds per-chunk logs |
| `apiBase` | `https://eclip5e.com/penumbra/` | hidden unless dev mode (Ctrl+Shift+D ×3 in About) |
| `offlineMode` | false | never touch network; play installed items |
| `cacheLimitMB` | 1024 | media/installer cache |
| Buttons | | Open logs folder, Clear cache, Reset settings, Export diagnostics (zip of logs + settings, no tokens) |

## Per item (`library.json` → `items.<id>.prefs`)
`channel`, `autoUpdate`, `launchArgs`, `env`, `runAsAdmin`, `cloudSaves`, `shareToken`, `hidden`.
Per-item prefs except paths sync as part of settings sync.
