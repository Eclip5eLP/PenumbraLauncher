# 05 — Launcher app (Tauri)

## Development
- `npm run tauri dev` (in `apps/launcher`): real app, Rust + Vite HMR.
- `npm run dev`: UI only in any browser at http://localhost:1420 — `src/lib/browser-mock.ts` stands in for the
  Rust backend (dev builds only, never bundled). Good for UI work and screenshots.
- `npm run check` (svelte-check), `cargo test --workspace`, `cargo clippy --workspace`.
- Release binary without installer: `npx tauri build --no-bundle` -> `target/release/penumbra-launcher.exe`.

## Rust backend modules (`apps/launcher/src-tauri/src/`)
| Module | Responsibility |
|--------|----------------|
| `main.rs` / `app.rs` | builder, plugins, window setup (hidden until UI ready), tray, single-instance, deep links |
| `config.rs` | paths (`%APPDATA%\Penumbra Launcher`, `%LOCALAPPDATA%\Penumbra Launcher\cache`), API base URL |
| `settings.rs` | load/save `settings.json` (atomic), schema version + migration, change events |
| `catalog.rs` | fetch with ETag, verify per-item signatures, disk cache, merge with local library state |
| `media.rs` | download/cache art to disk; UI loads via `asset://` / convertFileSrc so images never re-download |
| `auth.rs` | device flow, keyring storage, refresh with mutex, `authorized_request()` helper, sign-out |
| `library.rs` | `library.json`: installs (item, version, path, channel, size, last played, playtime) + rediscovery via `<install>/.penumbra/` |
| `jobs/` | `JobManager` actor, queue, job state machine, journal, progress aggregation |
| `jobs/download.rs` | HTTP chunk fetcher: N workers, retries w/ backoff + mirror failover, token-bucket rate limit |
| `jobs/apply.rs` | delta plan execution, staging, verify, atomic rename, deletions |
| `verify.rs` | full verify (BLAKE3, rayon), quick verify (size + mtime) |
| `process.rs` | launch (CreateProcess / ShellExecute runas), Job Object tracking, running state, playtime |
| `cloud.rs` | settings sync, cloud saves (zip save paths → userbase data API / saves.php) |
| `updater.rs` | wraps tauri-plugin-updater: check, download in background, "restart to update" |
| `commands.rs` | all `#[tauri::command]`s — thin, validated, delegate to services |
| `events.rs` | typed event names/payloads emitted to UI |
| `error.rs` | `thiserror` app error → serializable `{code, message}` for the UI |

## IPC contract (keep typed: generate TS types with `specta`/`tauri-specta`)
Commands (UI → Rust): `get_bootstrap()` (settings + cached catalog + library in one call = fast first paint),
`refresh_catalog`, `install(item, channel, path)`, `update(item)`, `pause/resume/cancel(job)`, `reorder_queue`,
`verify(item)`, `uninstall(item, keep_saves)`, `move_install(item, path)`, `locate_install(item, path)`,
`launch(item)`, `open_link(item)`, `open_folder(item)`, `get_settings/set_settings(patch)`,
`auth_start_device_login`, `auth_cancel`, `auth_sign_out`, `check_launcher_update`, `apply_launcher_update`,
`pick_folder`, `disk_space(path)`.

Events (Rust → UI): `catalog:updated`, `library:changed`, `job:progress` (≤10 Hz, batched for all jobs),
`job:state`, `auth:changed`, `auth:device_code`, `process:state`, `updater:state`, `settings:changed`, `toast`.

## Job state machine
`queued → preparing (fetch manifest, plan, space check) → downloading ⇄ paused → finalizing → done`
any → `error(code, retryable)` / `cancelled`. Updates and repairs are jobs too (`kind: install|update|repair|move`).
One job downloads at a time by default (setting: max concurrent jobs), others queued. Pausing persists across
restarts; "resume downloads on start" setting.

## Download engine details
- Workers: default 8 concurrent chunk requests (setting 1–32), HTTP/2 to one origin.
- Each chunk: GET → stream into memory (≤16 MB) → zstd decode → BLAKE3 verify → write at file offset
  (positioned writes, file preallocated with `set_len`). Bounded channel between fetch and write so memory stays
  < ~256 MB worst case.
- Retries: 5 with exponential backoff + jitter; on repeated failure switch mirror; hash mismatch = retry once then
  error `corrupt_chunk`.
- Rate limit: global token bucket shared by all workers.
- Progress: bytes downloaded, bytes to download, written files, speed (EMA 3 s), ETA.
- Pause while a game is running (setting).
- Disk: check free space (`needed = new files total - reusable + staging margin`) before start.

## Launching
- Resolve exe from install manifest/catalog; must be inside install dir (canonicalized).
- Args: catalog args + user args; env: user env + optional `PENUMBRA_ACCESS_TOKEN`/`PENUMBRA_USER` (opt-in per item
  via catalog `passToken` **and** user consent toggle).
- Before launch: if update available → dialog "Update now / Play anyway" (play-anyway allowed unless the catalog
  marks the version `required`). Quick verify (size check) optional.
- Running: game button shows "Running", Stop not offered by default (setting: show "Force close").
- Launcher behaviour on launch: minimize / hide to tray / stay / close (setting).

## Local files
```
%APPDATA%\Penumbra Launcher\        (roaming: small)
    settings.json   library.json   window.json
%LOCALAPPDATA%\Penumbra Launcher\   (local: big/regenerable)
    cache\catalog.json   cache\media\...   cache\installers\...   logs\launcher.log.*
<install dir>\.penumbra\           manifest.json   install.json   job.json   staging\
```
Tokens: Windows Credential Manager (`Penumbra Launcher/refresh_token`), never in files/logs.

## Performance
Budget (release build, mid-range PC, SSD):
- Window visible with real content (from cache): **< 1.0 s** after double-click; target 600 ms.
- Any click → visual response **< 50 ms**; page switch **< 100 ms**; no long task > 50 ms on the UI thread.
- Download at full speed must not drop UI below 60 fps: progress events ≤ 10 Hz, batched; hashing off the main runtime.
- Idle CPU ≈ 0 %. Idle RAM target < 150 MB total (WebView2 included). Background video pauses when minimized/unfocused.
- JS bundle < 200 KB gzipped. Fonts: one variable font, self-hosted, `font-display: swap`.

Techniques: window created `visible: false`, shown on the UI's first paint signal; `get_bootstrap` returns everything
in one IPC call; catalog/media from disk cache, network refresh after paint; images pre-sized WebP from the
publisher (icon 128, logo 800, bg 1920); crossfade by preloading the next background before switching;
CSS transforms/opacity only for animations; `prefers-reduced-motion` respected; release profile with
`lto = true`, `codegen-units = 1`, `opt-level = 3`, `panic = "abort"`, `strip = true`.

Measure: startup timer (process start → first paint) logged on every start; dev overlay with FPS + IPC timings
(Ctrl+Shift+D in dev builds).

## Security
- Tauri capabilities: only our commands; no `shell` open-any-URL from the UI (Rust opens only catalog URLs);
  `fs`/`http` plugins not exposed to the UI. Strict CSP (no remote scripts; images from asset protocol + our origin).
- All network: HTTPS, rustls. Manifests + catalog items verified with ed25519 before use.
- Never run anything not declared by a signed manifest/catalog entry.
