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
Each **channel** of a GitHub item picks its own mode, so one item can offer e.g. `stable` = latest release and
`dev` = latest pushed commit. The user switches channel in the item's gear menu (like any other channel).

```jsonc
"source": {
  "kind": "github",
  "repo": "Eclip5eLP/Pirate-Galaxy-AFK-Bot",
  "stripRoot": true,                    // drop the single top-level folder inside the archive
  "unpack": true,                       // false = keep the downloaded file as-is (e.g. a portable exe)
  "keep": ["config.json", "data/**"],   // user files never overwritten/deleted by updates
  "channels": {
    "stable": { "mode": "release", "asset": "PGBot-*-win64.zip" },
    "beta":   { "mode": "release", "asset": "PGBot-*-win64.zip", "prerelease": true },
    "dev":    { "mode": "branch",  "branch": "main" }
  }
}
```

### Modes
| Mode | Downloads | Version shown | New version when | Resume | Integrity |
|------|-----------|---------------|------------------|--------|-----------|
| `release` | asset matching `asset` glob of the latest release (`prerelease: true` includes pre-releases). If `asset` is omitted or nothing matches: the release's **source zip** (`releaseFile: "source"`) | release tag | a newer release is published | yes (Range) | asset sha256 digest |
| `branch` | zip of the **latest pushed commit** on `branch` (default: repo default branch) via `codeload.github.com/<repo>/zip/<sha>` | `<branch>@<short sha>` + commit date | a new commit is pushed | no (codeload zips are generated; restart on failure, fine for source-sized repos) | TLS + the zip is pinned to the exact commit sha |
| `tag` | source zip of the newest tag matching `tagPattern` (e.g. `v*`) — for repos that tag without making releases | tag name | a newer tag appears | no | TLS, pinned to the tag's commit |

Optional per-channel filters: `path` (only install a sub-folder of the repo/archive, e.g. `dist/`),
`exclude` (globs not to install, e.g. `.github/**`, `tests/**`).

### As implemented (M4)
- Channel settings (`mode`, `asset`, `prerelease`, `branch`, `tagPattern`, `path`, `exclude`) live in the item's
  `channels` map; `source` holds `repo`, `stripRoot`, `unpack`, `keep`. Item-level `launch` (exe/args, relative to
  the install) and `requires` replace the manifest's launch info for archive installs.
- `catalog.php` adds an unsigned top-level `resolved: { <item>: { <channel>: { version, url, download, sha256,
  date } } }` (server `api/v1/_github.php`). The launcher applies an entry only if its URL starts with
  `https://github.com/<repo>/releases/download/` or `https://codeload.github.com/<repo>/zip/` (ADR-022).
- No `resolved` key at all (old server) -> the launcher resolves against the GitHub API itself (`github.rs`).
- Install: `archive.rs` (download cache `%LOCALAPPDATA%\Penumbra Launcher\cache\downloads\<item>\`, deleted after
  install), local manifest (`"local": true`, hashes without chunks) in `.penumbra/manifest.json`.
- Launch targets that aren't `.exe` (e.g. `run_gui.py`) open through their Windows file association and aren't
  tracked as running (ADR-023).

### How updates are found
1. **Server resolver (primary):** `catalog.php` resolves every GitHub channel via the GitHub API
   (`releases`, `commits/<branch>`, `tags`) and embeds the result (`version`, download URL, size, `sha256` digest
   for assets, commit sha) into the item's channel entry. Results are cached in `penumbra/data/github-cache/`
   for 10 minutes (ETag requests, which don't count against the rate limit when unchanged), so all launchers
   together cost a few API calls per hour per repo. An optional server-side GitHub token (read-only, public
   repos, in the server's private config) raises the limit from 60 to 5000/h.
2. **Client fallback:** if the server didn't resolve (error/old cache), the launcher calls the GitHub API directly
   (unauthenticated, ETag-cached, at most once per hour per repo).
3. Installed GitHub items remember `{mode, tag|sha}`; "update available" = resolved tag/sha differs.

### Install / update flow
1. Download the asset to `%LOCALAPPDATA%\Penumbra Launcher\cache\downloads\` with HTTP Range resume
   (GitHub's CDN supports ranges; release asset URLs redirect to `objects.githubusercontent.com` /
   `release-assets.githubusercontent.com`).
2. Verify sha256 (asset digest). No digest (branch/tag zips, old releases) → TLS-only trust, pinned to the commit sha, logged.
3. Extract into `<install>/.penumbra/staging/`, compute BLAKE3 per file, write a *local* manifest in the same format
   as `penumbra` manifests (so verify/repair/uninstall code is shared).
4. Apply like a delta: only files whose hash changed are replaced, removed files deleted, `keep` respected.
5. Delete the cached archive (setting: keep last download for quick repair).

### Trust model (ADR-011)
The signed catalog entry pins `repo` + mode/asset pattern, not the bytes. Whoever can push or publish releases on that repo
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
- The user will add releases to the repo themselves. Item uses `stable` = `mode: release`.
- Until the first release exists, the item shows "No release yet" (catalog resolver returns no version).
  A `dev` channel with `mode: branch` is possible but needs Python 3.13 on the user's PC (`requires` check).
