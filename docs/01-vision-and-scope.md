# 01 — Vision & scope

## Vision
One fast, good-looking launcher for all Penumbra / eclip5e projects. It should feel like a mix of:
- **HoYoPlay** — full-bleed hero art per game, news carousel + posts bottom-left, one big action button bottom-right.
- **Battle.net** — left rail of game icons, version/channel selector next to the Play button, gear menu with
  verify/repair/uninstall.
- **Riot Client** — clean dark UI, smooth transitions, account presence top-right, download queue.
- **Collapse Launcher** — power-user settings (connections, bandwidth, repair, file-level verification), honest
  progress details (speed, ETA, bytes, files).

## Goals
1. Install, update, verify/repair, move and uninstall games and software from eclip5e.com.
2. Updates download only what changed and can be paused/resumed at any time, also across restarts.
3. Works for any content shape: one big zip, a folder of many small files, a single exe/installer.
4. Optional Penumbra Account sign-in: gated downloads, cloud saves, cross-device settings.
5. Web items: just open the link in the default browser.
6. The launcher updates itself (signed).
7. **Fast**: visible window < 1 s on SSD, no UI hitching while downloading. See budget in 05.

## Non-goals (for now)
- Linux/macOS (code stays portable where cheap, but no builds/testing).
- Store/payments, friends list/chat, achievements.
- Running web items inside the launcher.
- Third-party games not published by Penumbra.

## Item types
| Type | Example | Behaviour |
|------|---------|-----------|
| `game` | Pokemon Gracidea, Asperia Chronicles | Managed install (chunked), Play button, playtime, cloud saves possible |
| `software` | Ciphrixa | Same as game, labelled "Launch", grouped separately |
| `installer` | anything that must run its own setup | Download chunked installer to cache, run it, optionally detect install path afterwards |
| (any of the above) from GitHub | Pirate Galaxy AFK Bot | `source.kind = github`: release asset or branch zip, see 11-sources-github |
| `link` | web apps (PG Database, FireRPG, PenumbraTCG...) | "Open" → default browser. No install |

Every item has: id, name, type, source (`penumbra` | `github` | `url`), status (`released` | `coming-soon`), art (icon, logo, background image/video), description, news, channels
(e.g. `stable`, `beta`), access rule (`public`, `account`, `users:[...]`, `role:...`), visibility (listed/hidden).

## Feature list (full, prioritised in 10-roadmap)
- Catalog: grouped rail (Games / Software / Links), search, hide items, favourites/pin order, library grid view.
- Install: choose library folder, free-space check, size shown before install, queue, pause/resume/cancel,
  speed/ETA, bandwidth limit, concurrent connections, resume after crash/reboot.
- Update: delta (chunk-level), auto-update toggle per item, update before launch prompt, channels, rollback to
  previous version if the item allows it (optional, needs old manifests kept).
- Verify & repair (hash all files, redownload broken chunks only).
- Move install, locate existing install, uninstall (with "keep saves" option).
- Launch: exe + args + working dir, per-item user args/env, run as admin, process tracking (Running state,
  playtime), launcher behaviour on launch (minimize/tray/stay/close), pass account token to game (opt-in per item).
- Accounts: device sign-in (browser opens automatically), avatar/username top-right, sign out.
- Gated content: items visible but locked ("Sign in to download") or completely hidden unless entitled.
- Cloud: settings sync, cloud saves (declared save paths per item), playtime sync.
- News: per-item carousel banners + post list, global announcements.
- Self-update: background check, "restart to update" pill, stable/beta channel.
- Tray icon, start with Windows, start minimized.
- i18n: English only at first, but every UI string goes through the i18n layer from day one so German/Japanese
  can be added later without refactoring (no hard-coded strings, room for longer German labels, CJK font fallback).
- Offline mode: launch installed items without network.
