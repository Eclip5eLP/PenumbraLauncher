# Decisions (ADR log)

Format: number, date, decision, why, alternatives. Append only; supersede instead of editing.

**ADR-001 (2026-11-20) Tauri 2 + Rust core.** User choice. Small binary (~10 MB), fast startup, Rust is ideal for
the download/hash/patch engine. Alt: WinUI 3 (Windows-native like Collapse, heavier tooling), Electron (too heavy).

**ADR-002 (2026-11-20) Windows only.** User choice. Keep platform-specific code behind small modules
(`process.rs`, autostart, keyring) so Linux stays possible.

**ADR-003 (2026-11-20) Svelte 5 for the UI.** Compiled, no virtual DOM, tiny bundles, fine-grained reactivity →
fits the "no lag" goal; simpler to write than Solid. Plain CSS + tokens, no Tailwind/UI kit to keep CSS small and
the look custom.

**ADR-004 (2026-11-20) Content-defined chunking, files unpacked by the publisher.** Handles one-big-zip, loose
files and single exes with one format; delta updates, resume, dedup, CDN-cacheable pieces. Zips are unpacked so
the client needs no 2× disk space and updates aren't destroyed by zip compression. Alt: per-file diff (bad for one
big file), full archive (no delta), bsdiff patches (slow to build, need version pairs).

**ADR-005 (2026-11-20) BLAKE3 + zstd + FastCDC (avg 4 MiB).** BLAKE3 is fastest secure hash; zstd fast decode;
4 MiB avg balances request count vs. delta granularity and stays far below Cloudflare's per-file cache limit.
Option for later: bundle tiny files into shared chunks if items with thousands of tiny files show up.

**ADR-006 (2026-11-20) Static content + thin PHP.** Server already runs PHP 8.5/Apache; static files are served
efficiently and cached by Cloudflare; PHP only where auth/personalisation is needed. No new containers.

**ADR-007 (2026-11-20) Signing keys stay offline; per-item catalog signatures.** The server never holds a signing
key, so personalised catalogs are filtered lists of individually pre-signed item entries. A compromised server
cannot push code.

**ADR-008 (2026-11-20) Gating = gated manifests, public unguessable chunks.** Good enough for "account required"
downloads without putting PHP in the download path (keeps CDN caching). Not DRM.

**ADR-009 (2026-11-20) Device flow for sign-in.** Works today with the existing app `000007` without server changes;
browser opens automatically so it's nearly one-click. Loopback PKCE is a later, optional improvement.

**ADR-010 (2026-11-20) Publishing by CLI over SSH, catalog source in git.** Large uploads go over SSH (robust,
resumable with rsync, bypasses Cloudflare's 100 MB request limit). Catalog metadata is versioned in `catalog/`.
An admin web UI is optional later.

**ADR-011 (2026-11-20) GitHub as a download source.** Items can come from GitHub releases (or a branch zipball)
directly, using GitHub's bandwidth. Update discovery is resolved server-side (cached, optional token) with a client
fallback. Trust = signed catalog pins repo + asset pattern; bytes checked against GitHub's sha256 digest. No
chunk delta for these; files are diffed locally so unchanged files aren't rewritten. Private repos go through
`penumbra-pack import-github` instead (no tokens in the launcher).

**ADR-012 (2026-11-20) Public `main` / private `dev` with a one-way sync.** All work happens on `dev` (private
repo). `main` (public repo) receives snapshot commits built from `dev`'s tree minus the paths in `.publicignore`,
via `tools/sync-public.ps1`. `main` never shares history with `dev`, so private files can never leak through old
commits. Alt: two-way merges (private files leak via history), git subtree/filter-repo (heavy, error-prone).

**ADR-013 (2026-11-20) English only, i18n-ready.** All strings through the i18n layer from M1; German/Japanese later.
