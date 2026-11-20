# 10 — Roadmap

Each milestone ends with a working, committed state. Check boxes as work lands.

## M0 — Foundation
- [x] Plan + docs, git repo
- [x] Public `main` / private `dev` branches + `tools/sync-public.ps1`
- [x] GitHub remotes (public + private repos) pushed
- [ ] Cargo workspace, `penumbra-core` + `penumbra-pack` skeletons
- [ ] Tauri 2 + Svelte 5 + TS scaffold in `apps/launcher`, release profile, lint/format (rustfmt, clippy, prettier, svelte-check)
- [ ] Generate signing keys (manifest ed25519, updater minisign) into `C:\Eclip5eDev\keys\`

## M1 — UI shell (mock data)
- [ ] Frameless window, title bar, rail, item page, action button states, settings page skeleton, downloads page skeleton
- [ ] Design tokens, Inter font, icons, transitions, i18n scaffolding (en)
- [ ] `get_bootstrap` with mock catalog; window shown on first paint; startup timer logged
- [ ] Performance baseline measured and recorded in worklog

## M2 — Content pipeline + server
- [ ] `penumbra-core`: manifest types, path validation, FastCDC + BLAKE3 + zstd, sign/verify, tests
- [ ] `penumbra-pack build/upload/promote/list/verify`
- [ ] `server/penumbra`: .htaccess (cache headers, deny data/), `catalog.php`, `manifest.php`, `_lib.php`
- [ ] `tools/deploy-server.ps1`; deploy; Cloudflare caching of chunks verified (cf-cache-status: HIT)
- [ ] First real items published: Pokemon Gracidea (multi-part build), Asperia (coming-soon placeholder); a dev-only link item for testing

## M3 — Install & launch
- [ ] Real catalog fetch + cache + per-item signature check; media cache
- [ ] JobManager, downloader, apply, journal/resume, pause/cancel, queue
- [ ] Install dialog (path, space check), library.json, rediscover installs
- [ ] Launch + process tracking + playtime; link items open browser; installer items
- [ ] Verify & repair, uninstall, move, open folder
- [ ] Error handling + toasts + logs

## M4 — Updates
- [ ] Delta plan incl. local chunk reuse + `keep` rules; tests with synthetic versions
- [ ] Update detection, auto-update, update-before-play dialog, channels (stable/beta)
- [ ] Bandwidth limit, connections setting, pause while running
- [ ] `github` + `url` sources: per-channel modes (release / branch / tag), server resolver + cache, client fallback,
      range-resume, sha256, local manifest, `keep`
- [ ] `requires` check (Python) + Pirate Galaxy AFK Bot item
- [ ] `penumbra-pack import-github`

## M5 — Accounts
- [ ] Device sign-in dialog, keyring, refresh mutex, sign-out, account chip
- [ ] Personalised catalog, locked/restricted items, gated manifests

## M6 — Self-update & distribution
- [ ] NSIS config, updater plugin, `release.ps1`, `latest.json`
- [ ] Landing page `eclip5e.com/penumbra/` with download button
- [ ] End-to-end update test (v0.1.0 → v0.1.1)

## M7 — Cloud
- [ ] Settings sync
- [ ] Cloud saves (userbase data store), conflict dialog, local save backups
- [ ] Playtime sync

## M8 — Polish
- [ ] News carousel + posts (news.json per item), announcements
- [ ] Background video, Mica, tray menu, start with Windows, notifications
- [ ] Library grid, Ctrl+K quick switcher, rail reorder/pin/hide
- [ ] German / Japanese translations (later, not a focus)
- [ ] `penumbra://` deep links + "Install in launcher" buttons on the website
- [ ] Optional admin page for catalog/news editing
