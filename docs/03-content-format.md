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
- Content: zstd frame of the chunk data. Launcher verifies BLAKE3 of the **decompressed** data.
- Immutable: `Cache-Control: public, max-age=31536000, immutable`. `.bin` extension so Cloudflare caches it by
  default (to verify; a Cache Rule for `/penumbra/chunks/*` is the robust fix).
- Chunk params: FastCDC min 1 MiB / avg 4 MiB / max 16 MiB. Files < 1 MiB are one chunk each.
  (Many tiny files → many tiny requests; acceptable for now. Possible later: pack small files into shared
  "bundle chunks". Recorded as an option in decisions.md.)

## Manifest (one per item version)
Stored at `/penumbra/content/<item>/manifests/<version>.json.zst`, signed.

```jsonc
{
  "format": 1,
  "item": "gracidea",
  "version": "1.2.2",
  "created": "2026-11-20T20:00:00Z",
  "total_size": 734003200,          // installed size
  "download_size": 512000000,       // sum of compressed chunk sizes (full install)
  "launch": {                        // optional, can be overridden per channel in catalog
    "exe": "Pokemon - Gracidea Version.exe",
    "args": [], "workdir": ".", "admin": false
  },
  "files": [
    { "path": "Pokemon - Gracidea Version.exe", "size": 123456, "hash": "<blake3>",
      "exec": true,
      "chunks": [ { "hash": "<blake3>", "size": 123456, "csize": 60000 } ] }
  ],
  "dirs": ["Saves"],                 // empty dirs to create
  "keep": ["Saves/**", "*.ini"]      // never overwritten/deleted on update (user data); optional
}
```
Signature: detached ed25519 over the exact bytes of the uncompressed JSON, in `<version>.json.sig`
(and inline in the gated PHP response). Public key is compiled into the launcher; private key in
`C:\Eclip5eDev\keys\manifest_ed25519.key`.

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
```
penumbra-pack keygen                         # creates manifest signing keypair (once)
penumbra-pack build  --item gracidea --version 1.2.3 --input <dir|zip> [--no-unpack] [--exe ...]
                     -> out/<item>/<version>/ (manifest + new chunks only, compares with server chunk index)
penumbra-pack upload --item gracidea --version 1.2.3      # rsync/scp over SSH to the server, chunks first, manifest last
penumbra-pack promote --item gracidea --version 1.2.3 --channel stable   # edits catalog, signs, uploads catalog
penumbra-pack list   [--item ...]                          # versions per channel, sizes, chunk reuse stats
penumbra-pack gc     [--keep 3]                            # deletes chunks not referenced by kept versions
penumbra-pack verify --item ... --version ...              # checks that every chunk exists on the server
penumbra-pack import-github --item ... --tag ...           # GitHub release → penumbra version (private repos, delta)
```
- Knows existing chunks via `/penumbra/chunks/index.txt`-style listing fetched over SSH (`find`), so only new
  chunks are compressed and uploaded.
- Cross-platform: can run **on the server** (Linux build in Docker) to import zips that already live in `www/`
  (e.g. `development/Gracidea/data/PokemonGracidea.zip`) without re-uploading them through home upload.
- Upload order guarantees consistency: chunks → manifest → catalog. Clients never see a manifest whose chunks are
  missing.
