# 03 — Content format, delta updates, publisher

This doc covers the `penumbra` source (our own chunk CDN). GitHub/URL sources reuse the manifest format locally,
see [11-sources-github.md](11-sources-github.md).

Multi-part builds (e.g. Gracidea: exe zip + sprites zip + loose ini files) are assembled by the publisher into one
install tree via an item build file `catalog/<item>/build.toml` listing parts (`zip` → target folder, files →
target folder). One version = one manifest; dedup keeps part-only updates small.

## Why chunks
Most items are shipped today as one big zip, some as loose files. Requirements: resumable, verifiable, cheap
updates, cacheable by Cloudflare (single cached file must be < 512 MB, and smaller is better for parallelism and
retries). Solution: the publisher **unpacks** builds and stores every file as a list of content-defined chunks.
The launcher rebuilds files directly in the install folder — no extra disk space for a zip, no extraction step.

Content-defined chunking (FastCDC) means an edit in the middle of a big file only changes the chunks around the
edit, so updates of large asset packs are small. Identical chunks are stored once (dedup across versions and
across items).

Special cases:
- **Keep-archive mode**: an item can opt out of unpacking (`"unpack": false`), e.g. when the zip itself is the
  product. The zip is then one file in the manifest — still chunked, resumable and verified.
- **Single installer exe** (type `installer`): one-file manifest, downloaded to cache, then executed.

## Chunk store (server)
- Path: `/penumbra/chunks/<first 2 hex>/<blake3 hex of uncompressed data>.bin`
- Content: zstd frame (level 19) of the chunk data. Launcher verifies BLAKE3 of the **decompressed** data.
- Immutable: `Cache-Control: public, max-age=31536000, immutable`. Cloudflare caches `.bin` by default —
  verified 2026-11-20 (`cf-cache-status: HIT` on every chunk of a second download).
- Big files (>= 1 MiB): FastCDC min 1 MiB / avg 4 MiB / max 16 MiB.
- **Small files (< 1 MiB) are packed** (ADR-016): concatenated in path order into shared pack chunks, closed at
  4 MiB or, above 1 MiB, at a content-defined boundary (file hash first byte < 32). Gracidea: 5708 files -> 71
  chunks. A file inside a pack is a slice: `offset` + `size` into the pack, `len` = pack length.
- Constants live in `penumbra-core::chunk`.

## Manifest (one per item version)
Stored at `/penumbra/content/<item>/<version>.json.zst` (zstd) + `<version>.json.sig` (base64 ed25519 over the
exact uncompressed JSON bytes). Gated items will go to `data/content/` and only be served by `manifest.php`
(M5). Versions are **immutable**: `build` refuses a version that already exists on the server, because Cloudflare
edge-caches manifests for hours.

```jsonc
{
  "format": 1,
  "item": "gracidea",
  "version": "0.3.14+1.5.1+1.2.51",
  "created": "2026-11-20T22:10:00Z",
  "totalSize": 175798971,            // installed size
  "downloadSize": 146068838,         // sum of distinct compressed chunks (full install)
  "launch": { "exe": "Pokemon - Gracidea Version.exe", "args": [], "admin": false },
  "files": [
    { "path": "Pokemon - Gracidea Version.exe", "size": 4512768, "hash": "<blake3>", "exec": true,
      "chunks": [ { "hash": "<blake3>", "size": 4512768, "csize": 1800000 } ] },
    { "path": "items.ini", "size": 93687, "hash": "<blake3>",
      "chunks": [ { "hash": "<pack blake3>", "size": 93687, "csize": 2100000, "offset": 811008, "len": 4194304 } ] }
  ],
  "dirs": ["empty/dir"],             // empty dirs to create
  "keep": ["options.ini"]            // never overwritten/deleted on update (user data)
}
```
Public key: `keys/manifest_ed25519.pub` (in git, compiled into the launcher). Private key:
`C:\Eclip5eDev\keys\manifest_ed25519.key` (base64 seed; never in git). Catalog items are signed with the same key
over their canonical JSON (`penumbra-core::sign`, ADR-017).

Path rules (enforced in `penumbra-core::paths`, both at publish and install): relative, `/` separators, no `..`,
no drive letters, no reserved Windows names (CON, NUL, ...), no trailing dots/spaces, case-insensitive unique.

## Delta planning (`penumbra-core::delta`)
Input: installed manifest (or none), new manifest, local file states. Output plan:
1. Files whose `hash` matches the installed manifest and whose size/mtime match the install record → skip.
2. Files in `keep` that already exist → skip.
3. For every other file: for each chunk, source = (a) a chunk present in any local file of the old version
   (index old manifest: chunk hash → (file, offset)), else (b) download.
4. Files only in the old manifest → delete (unless in `keep`).
Writes go to `<install>/.penumbra/staging/<file>.part`; when complete and hash-verified, rename over the target.
Because old files are read for reuse before being replaced, files are processed so a target is replaced only after
all chunks that reference it as a source are copied (plan orders those reads first; fallback: download).

Journal: `<install>/.penumbra/job.json` lists completed files and per-file completed chunk ranges → resume after
crash re-verifies only partial files.

## Version identity
Semver-ish string chosen by the publisher; ordering comes from the catalog (`channels.<ch>.version`), not by
comparing strings. The installed manifest is kept in `<install>/.penumbra/manifest.json` (needed for delta and
verify, also offline).

## Publisher CLI: `penumbra-pack`
Run from the repo root. Config: `tools/private/pack.toml` (server ssh target, `run_as`, root path, public URL,
key paths) or `--config` / `$PENUMBRA_PACK_CONFIG`.

```
penumbra-pack keygen                                   # manifest/catalog keypair (once; refuses to overwrite)
penumbra-pack build   --item <id> --version <v> --input <dir|zip> [--level 19] [--offline]
penumbra-pack promote --item <id> --version <v> [--channel stable]   # sets channel, re-signs + stages catalog
penumbra-pack catalog                                  # re-sign + stage catalog after editing metadata/news/media
penumbra-pack upload  [--dry-run]                      # ships out/stage: chunks -> media -> manifests -> catalog
penumbra-pack list    [--item <id>]
penumbra-pack verify  --item <id> --version <v>        # all chunks + manifest present on the server?
penumbra-pack deploy-server                            # server/penumbra code -> server root (never deletes)
penumbra-pack install-test --item <id> [--channel stable] --dest <dir>   # end-to-end check via public URL
```
Not yet: `gc` (delete unreferenced chunks), `import-github` (M4).

### Local layout
- `catalog/catalog.json` (groups, mirrors, announcements, item order), `catalog/<id>/item.json` (metadata, media
  paths relative to `catalog/<id>/media/`), `catalog/<id>/build.toml` (optional build spec). `catalog/` is
  private-branch only (contains third-party art).
- `out/stage/` mirrors the server root; `upload` sends it as one tar stream over ssh and clears it.
- `out/<id>/<version>/manifest.json(.sig)` readable copies; `out/index.tsv` known chunks (refreshed from the
  server on every online build).

### Build spec (`catalog/<id>/build.toml`)
```toml
item = "gracidea"
exe = "Pokemon - Gracidea Version.exe"
keep = ["options.ini"]
[[part]]
zip = "PokemonGracidea.zip"        # relative to --input; extracted into `to` ("" = root)
[[part]]
files = ["items.ini", "moves.ini"]  # copied into `to`
[[part]]
zip = "maps.zip"
to = "maps"
# also: dir = "folder", strip_root = true (zips with one top folder)
```
Without a build spec, `--input` (folder or zip) is the install tree.

### Measured (Gracidea, 2026-11-20)
5708 files, 175.8 MB installed, 146.1 MB download in 71 chunks; build 20 s (zstd 19, all cores); upload 13 s on
LAN. Changing one ini file -> 1 new chunk (641 KB).
