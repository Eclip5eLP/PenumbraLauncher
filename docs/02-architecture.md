# 02 — Architecture

## Big picture

```
 ┌───────────────────────── Windows PC ─────────────────────────┐
 │  Penumbra Launcher (Tauri 2, one process + WebView2)          │
 │  ┌──────────────┐  IPC (commands + throttled events)          │
 │  │ Svelte 5 UI  │◄──────────────────────────────┐             │
 │  └──────────────┘                               │             │
 │  ┌────────────────────── Rust core ─────────────┴──────────┐  │
 │  │ catalog · auth · downloader · installer · verifier ·     │  │
 │  │ launcher/process · settings · cloud · updater · tray     │  │
 │  │            uses crates/penumbra-core                     │  │
 │  └──────────────────────────────────────────────────────────┘  │
 └───────────────┬───────────────────────────┬───────────────────┘
                 │ HTTPS                     │ HTTPS
   ┌─────────────▼──────────────┐   ┌────────▼──────────────────┐
   │ eclip5e.com/penumbra/      │   │ accounts.eclip5e.com      │
   │  static: chunks, manifests,│   │  (existing userbase)      │
   │  media, launcher updates   │   │  device flow, tokens,     │
   │  PHP: catalog, manifest    │   │  userinfo, data storage   │
   │  gate, news, saves, stats  │   └───────────────────────────┘
   └─────────────▲──────────────┘
                 │ SSH/rsync upload + catalog edits
   ┌─────────────┴──────────────┐
   │ penumbra-pack (publisher)  │  runs on the dev PC or on the server itself
   └────────────────────────────┘
```

Both hosts sit behind Cloudflare (tunnel → Nginx Proxy Manager → Apache/PHP 8.5 container `penumbra-web`).

## Tech choices (see decisions.md for the why)
| Area | Choice |
|------|--------|
| Shell | Tauri 2, NSIS installer, per-user install (no admin) |
| UI | Svelte 5 (runes) + TypeScript + Vite, plain CSS with design tokens, no UI framework |
| Rust async | tokio |
| HTTP | reqwest (rustls, HTTP/2, gzip off for chunks) |
| Hashing | BLAKE3 (fast, parallel) |
| Chunking | FastCDC (content-defined), avg 4 MiB, min 1 MiB, max 16 MiB |
| Compression | zstd per chunk (level 19 at publish time, decompression is fast) |
| Signing | ed25519 (manifests + catalog); minisign for Tauri updater |
| Token storage | Windows Credential Manager via `keyring` crate |
| Local state | JSON files with atomic write (settings, library); per-job journal files for downloads |
| Logging | `tracing` + rolling file appender, 7 days |
| Server | PHP 8.5 (existing container), static files served by Apache, cached by Cloudflare |

## Repo layout
```
/Cargo.toml                 workspace: crates/*, apps/launcher/src-tauri
/apps/launcher/
    package.json, vite.config.ts, svelte.config.js, index.html
    src/                    UI (routes, components, stores, i18n, styles)
    src-tauri/              Rust app: main.rs, commands/, services/, tauri.conf.json, capabilities/
/crates/penumbra-core/      manifest.rs, chunker.rs, hash.rs, delta.rs, sign.rs, paths.rs (validation)
/crates/penumbra-pack/      CLI (clap): build, upload, promote, list, gc, keygen, verify
/server/penumbra/           PHP API, .htaccess, landing page; deploy with tools/deploy-server.ps1
/catalog/                   items/<id>.json + art sources; published by penumbra-pack
/tools/                     deploy-server.ps1, release.ps1, dev-server.ps1
/docs/
```

## Data flow: install / update
1. UI asks `catalog.refresh()` → Rust fetches `catalog` (ETag/If-None-Match), verifies signature, caches to disk,
   emits `catalog:updated`. UI rendered from the disk cache first, so startup never waits on the network.
2. User clicks Install → `jobs.enqueue(item, channel, path)`.
3. Rust fetches the version's manifest (through the PHP gate if the item is gated), verifies signature.
4. `penumbra-core::delta::plan(local_state, manifest)` → list of files to write, chunks to fetch, chunks to
   copy from existing local files, files to delete.
5. Downloader fetches missing chunks in parallel, verifies BLAKE3, decompresses, writes into staging files.
6. Files are finalized (fsync + atomic rename), install record updated, staging cleaned.
7. Progress events (≤ 10 Hz) to the UI.

## Process model
- One main tokio runtime; CPU-heavy hashing/decompression on a rayon pool / `spawn_blocking` so IPC stays snappy.
- A single `JobManager` actor owns the download queue (mpsc commands: enqueue, pause, resume, cancel, reorder).
- `AuthManager` serializes token refresh (refresh tokens are single-use, rotating).
- `ProcessWatcher` tracks launched games via Windows Job Objects.
- Single-instance plugin: second launch focuses the existing window (and passes deep links later:
  `penumbra://install/<item>` registered as URL protocol — useful for "Install in launcher" buttons on the site).
