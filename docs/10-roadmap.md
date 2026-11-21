# 10 — Roadmap

Each milestone ends with a working, committed state. Check boxes as work lands.

## M0 — Foundation
- [x] Plan + docs, git repo
- [x] Public `main` / private `dev` branches + `tools/sync-public.ps1`
- [x] GitHub remotes (public + private repos) pushed
- [x] Cargo workspace, `penumbra-core` (catalog/manifest types, path rules + tests) + `penumbra-pack` CLI skeleton
- [x] Tauri 2 + Svelte 5 + TS scaffold in `apps/launcher`, release profile, svelte-check, prettier
- [x] Updater signing key (minisign) in `C:\Eclip5eDev\keys\`; manifest ed25519 key comes with `penumbra-pack keygen` (M2)

## M1 — UI shell (mock data)
- [x] Frameless window, title bar, rail, item page (background crossfade, news carousel, action panel), library grid,
      downloads page, settings page (persisted), install/uninstall dialogs, toasts
- [x] Design tokens, Inter font, Lucide icons, transitions, reduced motion, i18n scaffolding (en)
- [x] `get_bootstrap` with mock catalog; window shown on first paint; startup timer logged
- [x] Simulated job queue in Rust (pause/resume/cancel/prioritize, 10 Hz events) + browser dev mock
- [x] Performance baseline measured and recorded in worklog

## M2 — Content pipeline + server
- [x] `penumbra-core`: manifest types (+ pack slices), path validation, BLAKE3 + zstd chunk codec, ed25519 sign/verify,
      canonical JSON, tests
- [x] `penumbra-pack keygen/build/promote/catalog/upload/list/verify/deploy-server/install-test` (FastCDC + small-file
      packs, parallel zstd, immutable versions)
- [x] `server/penumbra`: .htaccess (cache headers, deny catalog/ + data/), `catalog.php` (ETag, access filter, token
      check via userbase), `manifest.php` (gating-ready), `_lib.php`, landing page
- [x] Deployed; Cloudflare caching of chunks + media verified (cf-cache-status: HIT)
- [x] Published: Pokemon Gracidea 0.3.14+1.5.1+1.2.51 (stable), Asperia (coming soon), PG Bot (GitHub, unresolved)
- [x] End-to-end install test via public URL: signatures valid, all 5708 file hashes match
- [x] Gracidea layout fixed from the GameMaker source; r2 published, starts without crashing
- [ ] Gated content staging into data/content (with M5), `gc`, dev-only link item (with M3 link support)

## M3 — Install & launch
- [x] Real catalog fetch (ETag) + disk cache + per-item signature check; media via WebView HTTP cache (immutable URLs)
- [x] JobManager, parallel downloader (connections setting, retries, bandwidth limit, pause while playing), staging,
      journal resume (also across restarts via jobs.json), pause/cancel/retry, queue
- [x] Install dialog (library folder picker, free space), library.json, `.penumbra/install.json` (+ `locate_install`)
- [x] Launch + Job Object tracking + playtime; link items open browser
- [x] Verify & repair (full hash, re-downloads only broken data), uninstall (manifest files only), open folder
- [x] Error handling: toasts, failed jobs with Retry, logs
- [x] E2E via CDP (tools/dev/cdp.mjs): install 146 MB in ~12 s, repair, launch/playtime, pause -> restart -> resume, cancel
- [ ] Move install, `installer` items, mirror failover, custom media cache for fully offline art (later)

## M4 — Updates
- [x] Delta plan incl. local chunk reuse + `keep` rules (unit tests + real r1 -> r2 update: 9.5 MB downloaded,
      124 MB reused locally, 5687 old files removed)
- [x] Update detection, auto-update (after every catalog refresh, every 30 min), "Play <old> without updating",
      channel switching; developer mode can install an older version (`install` with `version`)
- [x] Bandwidth limit, connections setting, pause while running (M3)
- [x] `github` + `url` sources: per-channel modes (release / branch / tag), server resolver (catalog `resolved` map,
      10 min cache), client fallback, Range resume, sha256 (when GitHub publishes one), local manifest, `keep`,
      strip root / sub-folder / excludes, repair
- [x] `requires` check (Python on PATH) + non-exe launch via file association; PG Bot `dev` channel installable
- [ ] `penumbra-pack import-github` (only needed for private repos — later)

## M5 — Accounts
- [x] Device sign-in dialog (code + copy + reopen browser), Credential Manager storage, serialized refresh with
      rotation, sign-out (server revoke), account chip menu, settings section
- [x] Personalised catalog (bearer + one retry on `X-Penumbra-Auth: invalid`), locked items -> sign-in, gated
      manifests via `manifest.php` with token; publisher stages non-public manifests to `data/content`
- [x] Admin-only test item `pltest` (role:admin): hidden + 401 for anonymous verified
- [x] Signed-in E2E: device sign-in (user Eclip5e, admin), personalised catalog (+pltest), gated install,
      restart -> silent refresh, sign-out (server revoke, credential + account.json removed, pltest hidden)

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
