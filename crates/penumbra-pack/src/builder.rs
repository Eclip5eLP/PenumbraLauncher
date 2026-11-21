//! `build`: assemble an install tree from a build spec, chunk it, compress new chunks into the stage folder,
//! write + sign the manifest (docs/03-content-format.md).

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::sync_channel;

use anyhow::{Context, Result, bail};
use penumbra_core::chunk::{self, CDC_AVG, CDC_MAX, CDC_MIN, PACK_MAX, PACK_MIN, SMALL_FILE};
use penumbra_core::manifest::{ChunkRef, FileEntry, Launch, MANIFEST_FORMAT, Manifest};
use penumbra_core::paths::validate_rel_path;
use serde::Deserialize;

use crate::workspace::Workspace;

/// `catalog/<item>/build.toml`
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct BuildSpec {
    pub item: String,
    /// Launch executable, relative to the install root.
    pub exe: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Globs of user files updates must never overwrite/delete.
    #[serde(default)]
    pub keep: Vec<String>,
    #[serde(default, rename = "part")]
    pub parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// A zip (relative to the input folder) to extract.
    pub zip: Option<String>,
    /// A folder (relative to the input folder) to copy.
    pub dir: Option<String>,
    /// Single files (relative to the input folder) to copy.
    #[serde(default)]
    pub files: Vec<String>,
    /// Target folder inside the install root ("" = root).
    #[serde(default)]
    pub to: String,
    /// Zips only: drop a single top-level folder.
    #[serde(default)]
    pub strip_root: bool,
}

pub struct BuildArgs {
    pub version: String,
    pub input: PathBuf,
    pub spec: BuildSpec,
    pub level: i32,
    pub offline: bool,
    /// Store the manifest privately even if the item is public (e.g. a restricted beta channel).
    pub gated: bool,
}

pub fn build(ws: &Workspace, args: BuildArgs) -> Result<()> {
    let item = &args.spec.item;
    if !chunk::is_valid_item_id(item) {
        bail!("invalid item id `{item}` (lowercase letters, digits, '-')");
    }
    if !chunk::is_valid_version(&args.version) {
        bail!("invalid version `{}` (letters, digits, . _ + -)", args.version);
    }
    let vdir = ws.version_dir(item, &args.version);
    let tree = vdir.join("tree");
    if tree.exists() {
        std::fs::remove_dir_all(&tree)?;
    }
    std::fs::create_dir_all(&tree)?;

    eprintln!("[1/4] assembling install tree in {}", tree.display());
    assemble(&args.spec, &args.input, &tree)?;

    eprintln!("[2/4] loading chunk index");
    let mut known = ws.load_index()?;
    if !args.offline {
        // Published versions are immutable: Cloudflare caches manifests at the edge for hours.
        let server = ws.config()?.server;
        let exists = server.run(&format!(
            "(test -e '{0}/content/{item}/{1}.json.zst' || test -e '{0}/data/content/{item}/{1}.json.zst') && echo yes || true",
            server.root, args.version
        ))?;
        if exists.trim() == "yes" {
            bail!("{item} {} is already published; versions are immutable, use a new version", args.version);
        }
        let remote = server.chunk_index()?;
        eprintln!("      {} chunks on server", remote.len());
        // The server is the truth; local-only entries (e.g. a deleted stage) must not hide missing chunks.
        known = remote;
        ws.save_index(&known)?;
    }

    eprintln!("[3/4] chunking (zstd level {})", args.level);
    let (files, dirs, new_chunks) = chunk_tree(ws, &tree, &known, args.level)?;

    eprintln!("[4/4] writing manifest");
    let mut csizes = known;
    csizes.extend(new_chunks.iter().map(|(h, c)| (h.clone(), *c)));
    // Staged chunks count as known for later builds (dedup across items/versions before uploading).
    ws.save_index(&csizes)?;
    let mut files = files;
    for f in &mut files {
        for c in &mut f.chunks {
            c.csize = *csizes.get(&c.hash).context("chunk size missing")?;
        }
    }
    let mut seen = HashSet::new();
    let download_size =
        files.iter().flat_map(|f| &f.chunks).filter(|c| seen.insert(c.hash.clone())).map(|c| c.csize).sum();
    let manifest = Manifest {
        format: MANIFEST_FORMAT,
        item: item.clone(),
        version: args.version.clone(),
        created: crate::now_rfc3339(),
        total_size: files.iter().map(|f| f.size).sum(),
        download_size,
        launch: args.spec.exe.as_ref().map(|exe| Launch {
            exe: exe.clone(),
            args: args.spec.args.clone(),
            workdir: None,
            admin: false,
        }),
        files,
        dirs,
        keep: args.spec.keep.clone(),
        local: false,
    };
    if let Some(l) = &manifest.launch
        && !manifest.files.iter().any(|f| f.path.eq_ignore_ascii_case(&l.exe))
    {
        bail!("launch exe `{}` is not part of the build", l.exe);
    }
    manifest.validate()?;
    let json = serde_json::to_vec(&manifest)?;
    let sig = penumbra_core::sign::sign_bytes(&ws.signing_key()?, &json);
    std::fs::write(vdir.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    std::fs::write(vdir.join("manifest.json.sig"), &sig)?;
    // Staged for upload: exactly the signed bytes, compressed.
    // Non-public items: the manifest lives in data/ (never served statically) and only manifest.php hands it out
    // after an access check. Chunks stay public but unguessable (ADR-008).
    let gated = args.gated || crate::catalog::item_access(ws, item).is_some_and(|a| a != "public");
    let stage_base = format!("{}content/{item}/{}", if gated { "data/" } else { "" }, args.version);
    if gated {
        eprintln!("      gated item: manifest goes to data/content (served only via api/v1/manifest.php)");
    }
    ws.stage_write(&format!("{stage_base}.json.zst"), &zstd::bulk::compress(&json, 19)?)?;
    ws.stage_write(&format!("{stage_base}.json.sig"), sig.as_bytes())?;
    std::fs::remove_dir_all(&tree).ok();

    let new_bytes: u64 = new_chunks.values().sum();
    eprintln!(
        "done: {} files, {} installed, {} download; {} new chunks ({}) staged. Next: `penumbra-pack upload`.",
        manifest.files.len(),
        crate::human(manifest.total_size),
        crate::human(manifest.download_size),
        new_chunks.len(),
        crate::human(new_bytes)
    );
    Ok(())
}

fn assemble(spec: &BuildSpec, input: &Path, tree: &Path) -> Result<()> {
    if spec.parts.is_empty() {
        // No parts: the input itself (folder or zip) is the install tree.
        if input.is_dir() {
            copy_dir(input, tree)?;
        } else {
            extract_zip(input, tree, false)?;
        }
        return Ok(());
    }
    for p in &spec.parts {
        let to = if p.to.is_empty() { tree.to_path_buf() } else {
            validate_rel_path(&p.to)?;
            tree.join(&p.to)
        };
        std::fs::create_dir_all(&to)?;
        if let Some(z) = &p.zip {
            eprintln!("      zip  {z} -> /{}", p.to);
            extract_zip(&input.join(z), &to, p.strip_root)?;
        }
        if let Some(d) = &p.dir {
            eprintln!("      dir  {d} -> /{}", p.to);
            copy_dir(&input.join(d), &to)?;
        }
        for f in &p.files {
            let src = input.join(f);
            let name = Path::new(f).file_name().context("bad file name")?;
            std::fs::copy(&src, to.join(name)).with_context(|| format!("copying {}", src.display()))?;
        }
        if !p.files.is_empty() {
            eprintln!("      {} files -> /{}", p.files.len(), p.to);
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, to: &Path, strip_root: bool) -> Result<()> {
    let mut zip = zip::ZipArchive::new(BufReader::new(
        File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?,
    ))?;
    let root = if strip_root {
        let first = zip.by_index(0)?.enclosed_name().context("unsafe zip entry")?;
        Some(first.components().next().context("empty zip")?.as_os_str().to_owned())
    } else {
        None
    };
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        let Some(mut rel) = e.enclosed_name() else { bail!("unsafe path in zip: {:?}", e.name()) };
        if let Some(r) = &root {
            rel = rel.strip_prefix(r).map(|p| p.to_path_buf()).unwrap_or(rel);
            if rel.as_os_str().is_empty() {
                continue;
            }
        }
        let dest = to.join(&rel);
        if e.is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut e, &mut File::create(&dest)?)?;
        }
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for e in walkdir::WalkDir::new(from) {
        let e = e?;
        let rel = e.path().strip_prefix(from)?;
        let dest = to.join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

type Chunked = (Vec<FileEntry>, Vec<String>, HashMap<String, u64>);

/// Walks the tree in a stable order, cuts big files with FastCDC and packs small files together. New chunks
/// are compressed in parallel (bounded memory) into the stage folder. Returns files (csize still 0), empty
/// dirs and the newly staged chunks (hash -> compressed size).
fn chunk_tree(ws: &Workspace, tree: &Path, known: &HashMap<String, u64>, level: i32) -> Result<Chunked> {
    let mut paths = vec![];
    let mut dirs = vec![];
    for e in walkdir::WalkDir::new(tree).min_depth(1) {
        let e = e?;
        let rel = e.path().strip_prefix(tree)?.to_string_lossy().replace('\\', "/");
        if e.file_type().is_dir() {
            if std::fs::read_dir(e.path())?.next().is_none() {
                dirs.push(rel);
            }
        } else {
            validate_rel_path(&rel).with_context(|| format!("file `{rel}`"))?;
            paths.push((rel, e.path().to_path_buf()));
        }
    }
    paths.sort_by_key(|(rel, _)| rel.to_lowercase());

    let staged: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());
    let in_flight: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let slots = rayon::current_num_threads().max(2) * 2;
    let (permit_tx, permit_rx) = sync_channel::<()>(slots);
    for _ in 0..slots {
        permit_tx.send(()).unwrap();
    }
    let mut files: Vec<FileEntry> = Vec::with_capacity(paths.len());
    let error: Mutex<Option<anyhow::Error>> = Mutex::new(None);

    // in_place_scope: the producer (file reading/chunking) runs on this thread, so blocking on a permit never
    // occupies a pool worker that compression tasks need.
    rayon::in_place_scope(|s| -> Result<()> {
        // Hands a finished chunk to the compression pool unless it already exists.
        let mut emit = |hash: String, data: Vec<u8>| {
            if known.contains_key(&hash) || !in_flight.lock().unwrap().insert(hash.clone()) {
                return;
            }
            permit_rx.recv().unwrap();
            let (staged, error, permit_tx) = (&staged, &error, permit_tx.clone());
            s.spawn(move |_| {
                let r = (|| -> Result<u64> {
                    let z = zstd::bulk::compress(&data, level)?;
                    ws.stage_write(&chunk::chunk_path(&hash), &z)?;
                    Ok(z.len() as u64)
                })();
                match r {
                    Ok(c) => {
                        staged.lock().unwrap().insert(hash, c);
                    }
                    Err(e) => *error.lock().unwrap() = Some(e),
                }
                permit_tx.send(()).unwrap();
            });
        };

        let mut pack: Vec<u8> = Vec::new();
        let mut members: Vec<(usize, u64, u64)> = vec![]; // (file index, offset, size)
        let close_pack = |pack: &mut Vec<u8>, members: &mut Vec<(usize, u64, u64)>, files: &mut Vec<FileEntry>, emit: &mut dyn FnMut(String, Vec<u8>)| {
            if pack.is_empty() {
                return;
            }
            let hash = chunk::hash_hex(pack);
            let len = pack.len() as u64;
            for &(idx, offset, size) in members.iter() {
                let single = members.len() == 1;
                files[idx].chunks.push(ChunkRef {
                    hash: hash.clone(),
                    size,
                    csize: 0,
                    offset,
                    len: if single { None } else { Some(len) },
                });
            }
            members.clear();
            emit(hash, std::mem::take(pack));
        };

        let total = paths.len();
        for (i, (rel, path)) in paths.iter().enumerate() {
            let size = std::fs::metadata(path)?.len();
            let exec = rel.to_lowercase().ends_with(".exe");
            if size == 0 {
                files.push(FileEntry { path: rel.clone(), size: 0, hash: chunk::hash_hex(b""), exec, chunks: vec![] });
            } else if size < SMALL_FILE {
                let data = std::fs::read(path)?;
                let hash = chunk::hash_hex(&data);
                files.push(FileEntry { path: rel.clone(), size, hash: hash.clone(), exec, chunks: vec![] });
                members.push((files.len() - 1, pack.len() as u64, size));
                pack.extend_from_slice(&data);
                if pack.len() as u64 >= PACK_MAX || (pack.len() as u64 >= PACK_MIN && chunk::is_pack_boundary(&hash)) {
                    close_pack(&mut pack, &mut members, &mut files, &mut emit);
                }
            } else {
                let mut hasher = blake3::Hasher::new();
                let mut refs = vec![];
                let reader = BufReader::with_capacity(1 << 20, File::open(path)?);
                for c in fastcdc::v2020::StreamCDC::new(reader, CDC_MIN as usize, CDC_AVG as usize, CDC_MAX as usize) {
                    let c = c.with_context(|| format!("chunking {rel}"))?;
                    hasher.update(&c.data);
                    let h = chunk::hash_hex(&c.data);
                    refs.push(ChunkRef { hash: h.clone(), size: c.length as u64, csize: 0, offset: 0, len: None });
                    emit(h, c.data);
                }
                files.push(FileEntry { path: rel.clone(), size, hash: hasher.finalize().to_hex().to_string(), exec, chunks: refs });
            }
            if (i + 1) % 250 == 0 || i + 1 == total {
                eprint!("\r      {}/{} files", i + 1, total);
            }
        }
        close_pack(&mut pack, &mut members, &mut files, &mut emit);
        eprintln!();
        Ok(())
    })?;

    if let Some(e) = error.into_inner().unwrap() {
        return Err(e);
    }
    Ok((files, dirs, staged.into_inner().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: u64, len: usize) -> Vec<u8> {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn chunked_tree_rebuilds_byte_for_byte() {
        let base = std::env::temp_dir().join(format!("pgl-pack-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let tree = base.join("tree");
        std::fs::create_dir_all(tree.join("data/sub")).unwrap();
        std::fs::create_dir_all(tree.join("empty-dir")).unwrap();
        let mut originals = HashMap::new();
        for i in 0..300u64 {
            let len = (i * 7919 % 200_000) as usize;
            let rel = format!("data/sub/f{i:03}.bin");
            let bytes = noise(i, len);
            std::fs::write(tree.join(&rel), &bytes).unwrap();
            originals.insert(rel, bytes);
        }
        let big = noise(999, 9 << 20);
        std::fs::write(tree.join("big.pak"), &big).unwrap();
        originals.insert("big.pak".into(), big);
        std::fs::write(tree.join("zero.txt"), b"").unwrap();
        originals.insert("zero.txt".into(), vec![]);

        let ws = Workspace::new(base.clone(), None);
        let (files, dirs, staged) = chunk_tree(&ws, &tree, &HashMap::new(), 3).unwrap();
        assert_eq!(dirs, vec!["empty-dir".to_string()]);
        assert_eq!(files.len(), originals.len());

        let small_files = files.iter().filter(|f| f.size > 0 && f.size < SMALL_FILE).count();
        let small_chunks: HashSet<_> = files
            .iter()
            .filter(|f| f.size > 0 && f.size < SMALL_FILE)
            .flat_map(|f| f.chunks.iter().map(|c| c.hash.clone()))
            .collect();
        assert!(small_chunks.len() * 5 < small_files, "small files should be packed: {} chunks for {small_files} files", small_chunks.len());

        for f in &files {
            let mut rebuilt = vec![];
            for c in &f.chunks {
                assert!(staged.contains_key(&c.hash));
                let stored = std::fs::read(ws.stage().join(chunk::chunk_path(&c.hash))).unwrap();
                let data = chunk::decode_chunk(&stored, &c.hash, c.chunk_len()).unwrap();
                rebuilt.extend_from_slice(&data[c.offset as usize..(c.offset + c.size) as usize]);
            }
            assert_eq!(&rebuilt, &originals[&f.path], "{}", f.path);
            assert_eq!(chunk::hash_hex(&rebuilt), f.hash);
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
