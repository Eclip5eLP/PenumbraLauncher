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

**ADR-014 (2026-11-20) GitHub mode per channel.** A GitHub item's channels each choose `release` (latest release
asset or source zip), `branch` (latest pushed commit) or `tag`. Reuses the existing channel switcher instead of a
separate setting, and lets one item offer stable releases and bleeding-edge builds side by side.

**ADR-015 (2026-11-20) No license file.** User chose GitHub's default (no license = all rights reserved; the public
repo is source-available, not open source). Revisit if contributions or reuse are wanted.

**ADR-016 (2026-11-20) Small files are packed into shared chunks.** Gracidea has 5683 files under 1 MiB; one
request per file would make installs slow over Cloudflare. Files < 1 MiB are concatenated (path order) into packs
closed at 4 MiB or at a content-defined boundary above 1 MiB; a file is a slice (`offset`, `size`, `len`).
Result: 5708 files -> 71 chunks; editing one file re-downloads one pack (~0.6-4 MB). Supersedes the "later" note
in ADR-005.

**ADR-017 (2026-11-20) Catalog = signed JSON lines, passed through byte-for-byte.** Each item is signed over its
canonical JSON (sorted keys, no whitespace; implemented by hand so serde_json's `preserve_order` feature can't
change it). The server stores `items.jsonl` and only filters lines by access rule, never re-encodes (PHP would turn
`{}` into `[]`). Media get content-hashed names so they can be cached forever.

**ADR-018 (2026-11-20) Published versions are immutable.** Cloudflare edge-caches manifests for hours (its browser
TTL overrides our 5 min), so a version string is never reused; `build` checks the server and refuses.

**ADR-019 (2026-11-21) Staging + journal instead of in-place writes.** Files are assembled in
`.penumbra/staging` and only renamed into place after every staged file hashes correctly, so a paused, crashed or
failed update never leaves a half-updated game. `journal.txt` (first line = target version) lists finished chunks;
resuming re-plans deterministically and skips them. Costs temporary disk space = size of changed files.

**ADR-020 (2026-11-21) native-tls (SChannel) for HTTPS.** reqwest 0.13's default rustls backend uses aws-lc-rs,
which can need extra build tools on Windows; SChannel needs none and uses the Windows certificate store.

**ADR-021 (2026-11-21) Media via the WebView HTTP cache.** Media URLs are content-hashed and immutable, so WebView2's
own disk cache keeps them; no custom media cache yet. Revisit if art must show on a cold, offline start.

**ADR-022 (2026-11-21) GitHub resolution travels unsigned next to signed items.** The server can't sign (ADR-007),
and GitHub versions change without a publish step. So `resolved` is a separate unsigned map, and the launcher
accepts only URLs inside the repository the signed item names. Worst case a compromised server can point at an
older/other file *of that same repository*.

**ADR-023 (2026-11-21) Non-exe launch targets use the file association, untracked.** Python apps (PG Bot from
source) start via `run_gui.py`'s association (py launcher). Tracking would need knowing the interpreter; not worth
it before the bot ships exe releases. `requires: python` is checked first.

**ADR-024 (2026-11-21) Credential Manager via windows-sys, not the keyring crate.** keyring 4 just changed its API
(keyring-core stores); the three Win32 calls we need (CredWriteW/CredReadW/CredDeleteW) are ~60 lines and have no
moving parts.
