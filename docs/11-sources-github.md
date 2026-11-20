# 11 — Download sources (Penumbra CDN, GitHub, plain URL)

Every installable catalog item has a `source`. The launcher's job pipeline is the same for all of them
(queue, progress, pause/resume, verify, launch); only "where do the bytes come from and how are updates found"
differs.

| `source.kind` | Bytes come from | Updates | Delta | Signed | Use for |
|---------------|-----------------|---------|-------|--------|---------|
| `penumbra` (default) | `eclip5e.com/penumbra/chunks` (Cloudflare cached) | catalog channel version | chunk-level | manifest ed25519 | own games, big content |
| `github` | GitHub release asset or source archive | latest release / branch head | file-level only | trust GitHub repo + sha256 digest | tools/bots/apps whose builds live on GitHub |
| `url` | any HTTPS zip/exe URL | version string in catalog | file-level only | sha256 in catalog entry | legacy downloads, third-party mirrors |

## `github` source
```jsonc
"source": {
  "kind": "github",
  "repo": "Eclip5eLP/Pirate-Galaxy-AFK-Bot",
  "mode": "release",               // "release" (recommended) | "ref"
  "asset": "PGBot-*-win64.zip",    // glob over release asset names (release mode)
  "prerelease": false,             // beta channel may set true
  "ref": "main",                   // ref mode only: branch / tag / commit → GitHub zipball
  "stripRoot": true,               // drop the single top-level folder inside the archive
  "unpack": true,                  // false = keep the downloaded file as-is (e.g. a portable exe)
  "keep": ["config.json", "data/**"]   // user files never overwritten/deleted by updates
}
```

### How updates are found
1. **Server resolver (primary):** `catalog.php` resolves GitHub items via the GitHub API and embeds the result
   (`version` = tag or short commit sha, asset URL, size, `sha256` from the asset's `digest`) into the item's
   channel entry. Results are cached in `penumbra/data/github-cache/` for 10 minutes, so all launchers together
   cost ~6 API calls/hour per repo. An optional server-side GitHub token (in `private/`, read-only, public repos)
   raises the limit from 60 to 5000/h.
2. **Client fallback:** if the server didn't resolve (error/old cache), the launcher calls the GitHub API directly
   (unauthenticated, ETag-cached, at most once per hour per repo).

### Install / update flow
1. Download the asset to `%LOCALAPPDATA%\Penumbra Launcher\cache\downloads\` with HTTP Range resume
   (GitHub's CDN supports ranges; release asset URLs redirect to `objects.githubusercontent.com` /
   `release-assets.githubusercontent.com`).
2. Verify sha256 (asset digest). Missing digest (old releases / zipballs) → TLS-only trust, logged.
3. Extract into `<install>/.penumbra/staging/`, compute BLAKE3 per file, write a *local* manifest in the same format
   as `penumbra` manifests (so verify/repair/uninstall code is shared).
4. Apply like a delta: only files whose hash changed are replaced, removed files deleted, `keep` respected.
5. Delete the cached archive (setting: keep last download for quick repair).

### Trust model (ADR-011)
The signed catalog entry pins `repo` + `asset` pattern, not the bytes. Whoever can publish releases on that repo
can ship code to launcher users. That's acceptable for the user's own repos (2FA on the GitHub account matters).

### Private repos
Never put GitHub tokens into the launcher. For private repos (or when delta updates matter) use **import mode**:
`penumbra-pack import-github --item <id> --tag <tag>` downloads the release (with the publisher's own `gh` auth),
converts it into a normal `penumbra` version (chunks + signed manifest). Can run on the server on a schedule
later (cron: poll releases → import → promote).

## Runtime requirements (e.g. Python apps)
Items can declare prerequisites the launcher checks before launch:
```jsonc
"requires": [ { "kind": "python", "version": ">=3.13", "help": "https://www.python.org/downloads/windows/" } ]
```
Phase 1 only *checks* and shows a helpful "Python 3.13 required — Install" dialog. Bundling runtimes is a
non-goal; the better fix is shipping a frozen build (PyInstaller/Nuitka exe built by GitHub Actions on each tag).

## Initial GitHub item: Pirate Galaxy AFK Bot
- Repo `Eclip5eLP/Pirate-Galaxy-AFK-Bot` (public, Python 3.13, entry `run_gui.py`), currently **no releases/tags**.
- Recommended: add a GitHub Actions workflow to that repo that on tag `v*` builds `PGBot-<ver>-win64.zip`
  with PyInstaller and attaches it to the release. Launcher item then uses `mode: release`.
- Until then: `mode: ref, ref: main` + `requires: python>=3.13`, launch `pythonw.exe run_gui.py`
  (after `pip install -r requirements.txt` into a per-item venv — only if we choose to support that; see
  open questions).
