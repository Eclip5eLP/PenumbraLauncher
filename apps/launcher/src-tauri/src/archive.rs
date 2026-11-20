//! Installs from GitHub releases/branches/tags and plain URLs (docs/11-sources-github.md).
//!
//! download (Range resume when the server allows) -> sha256 check (when known) -> unpack into
//! `.penumbra/staging-tree` (strip root folder, `path` sub-folder, `exclude` globs) -> local manifest (BLAKE3 per
//! file) -> move only changed files into place, keep user files, delete removed files.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use penumbra_core::catalog::{Item, Source, github_url_allowed};
use penumbra_core::manifest::{FileEntry, MANIFEST_FORMAT, Manifest};
use penumbra_core::paths::validate_rel_path;
use penumbra_core::plan::glob_match;
use sha2::Digest;
use tauri::{AppHandle, Manager};

use crate::engine::{Kind, Outcome, PHASE_DOWNLOADING, PHASE_FINALIZING, PHASE_PREPARING, Progress, Spec};
use crate::library::{self, InstallRecord};
use crate::paths;
use crate::state::AppState;

type R<T> = Result<T, String>;

struct Opts {
    unpack: bool,
    strip_root: bool,
    keep: Vec<String>,
    sub: Option<String>,
    exclude: Vec<String>,
}

pub async fn run(app: AppHandle, spec: Spec, item: Item, prog: Arc<Progress>) -> R<Outcome> {
    prog.phase.store(PHASE_PREPARING, Ordering::Relaxed);
    let ch = item.channels.get(&spec.channel).cloned().ok_or("channel no longer exists")?;
    let version = ch.version.clone().ok_or("nothing published on this channel")?;
    let url = ch.url.clone().ok_or("no download for this version yet")?;
    let opts = match &item.source {
        Some(Source::Github { repo, strip_root, unpack, keep }) => {
            if !github_url_allowed(repo, &url) {
                return Err("download URL is outside the item's GitHub repository".into());
            }
            Opts { unpack: *unpack, strip_root: *strip_root, keep: keep.clone(), sub: ch.path.clone(), exclude: ch.exclude.clone() }
        }
        Some(Source::Url { unpack, keep }) => {
            if !url.starts_with("https://") {
                return Err("only https downloads are allowed".into());
            }
            Opts { unpack: *unpack, strip_root: true, keep: keep.clone(), sub: ch.path.clone(), exclude: ch.exclude.clone() }
        }
        _ => return Err("not an archive source".into()),
    };

    // 1. Download (cached per version, resumable).
    let dir = paths::local_dir().join("cache").join("downloads").join(&item.id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cache folder: {e}"))?;
    let safe: String = version.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    let file = dir.join(format!("{safe}.download"));
    if !file.exists() {
        download(&app, &url, &file, ch.download.unwrap_or(0), &prog).await?;
    }
    if let Some(want) = &ch.sha256 {
        let f = file.clone();
        let got = tokio::task::spawn_blocking(move || sha256_file(&f)).await.map_err(|e| e.to_string())??;
        if !got.eq_ignore_ascii_case(want) {
            let _ = std::fs::remove_file(&file);
            return Err("download doesn't match its published checksum; please try again".into());
        }
    } else {
        tracing::info!(item = %item.id, "no published checksum; trusting TLS + pinned repository");
    }

    // 2+3. Unpack, compare, apply (blocking).
    prog.phase.store(PHASE_FINALIZING, Ordering::Relaxed);
    let (install, id, launch, deep) = (spec.path.clone(), item.id.clone(), item.launch.clone(), spec.kind == Kind::Repair);
    let (ver2, file2, url2, ch_name) = (version.clone(), file.clone(), url.clone(), spec.channel.clone());
    let (written, size) = tokio::task::spawn_blocking(move || -> R<(usize, u64)> {
        let meta = library::meta_dir(&install);
        let tree = meta.join("staging-tree");
        let _ = std::fs::remove_dir_all(&tree);
        std::fs::create_dir_all(&tree).map_err(|e| format!("staging: {e}"))?;
        let name = url2.rsplit('/').next().unwrap_or("download").to_string();
        let files = unpack(&file2, &tree, &name, &opts)?;
        let manifest = Manifest {
            format: MANIFEST_FORMAT,
            item: id.clone(),
            version: ver2.clone(),
            created: String::new(),
            total_size: files.iter().map(|f| f.size).sum(),
            download_size: std::fs::metadata(&file2).map(|m| m.len()).unwrap_or(0),
            launch,
            files,
            dirs: vec![],
            keep: opts.keep.clone(),
            local: true,
        };
        let written = apply(&manifest, &install, &tree, deep)?;
        std::fs::write(meta.join("manifest.json"), serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("writing manifest: {e}"))?;
        let rec = InstallRecord { item: id, version: ver2, channel: ch_name };
        let _ = std::fs::write(meta.join("install.json"), serde_json::to_vec_pretty(&rec).unwrap_or_default());
        let _ = std::fs::remove_dir_all(&tree);
        Ok((written, manifest.total_size))
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(&file); // installed: the archive isn't needed anymore

    let note = match spec.kind {
        Kind::Repair if written == 0 => Some("All files are OK.".to_string()),
        Kind::Repair => Some(format!("Repaired {written} file{}.", if written == 1 { "" } else { "s" })),
        _ => None,
    };
    Ok(Outcome { size, written, note, version: Some(version) })
}

async fn download(app: &AppHandle, url: &str, dest: &Path, expected: u64, prog: &Progress) -> R<()> {
    let state = app.state::<AppState>();
    let part = dest.with_extension("part");
    let mut last = String::new();
    prog.phase.store(PHASE_DOWNLOADING, Ordering::Relaxed);
    for attempt in 0..5u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 << attempt)).await;
        }
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let mut req = state.http.get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut resp = match req.send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                last = format!("server answered {}", r.status());
                continue;
            }
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let resumed = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let start = if resumed { have } else { 0 };
        let total = resp.content_length().map(|l| l + start).unwrap_or(expected);
        prog.total.store(total, Ordering::Relaxed);
        prog.done.store(start, Ordering::Relaxed);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&part)
            .map_err(|e| format!("download file: {e}"))?;
        let result: R<()> = async {
            while let Some(piece) = resp.chunk().await.map_err(|e| e.to_string())? {
                crate::engine::wait_while_game_runs(&state).await;
                let limit = state.settings.lock().unwrap().bandwidth_limit;
                state.limiter.take(piece.len(), limit).await;
                f.write_all(&piece).map_err(|e| format!("writing download: {e}"))?;
                prog.done.fetch_add(piece.len() as u64, Ordering::Relaxed);
                if prog.stop.load(Ordering::Relaxed) {
                    return Err("stopped".into());
                }
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                drop(f);
                if prog.total.load(Ordering::Relaxed) == 0 {
                    prog.total.store(prog.done.load(Ordering::Relaxed), Ordering::Relaxed);
                }
                return std::fs::rename(&part, dest).map_err(|e| format!("download file: {e}"));
            }
            Err(e) => last = e,
        }
    }
    Err(format!("download failed: {last}"))
}

fn sha256_file(p: &Path) -> R<String> {
    let mut h = sha2::Sha256::new();
    let mut f = std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(p).map_err(|e| e.to_string())?);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Hashes while copying.
fn copy_hashed(mut from: impl Read, to: &Path) -> R<(u64, String)> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(to).map_err(|e| e.to_string())?);
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n_total = 0u64;
    loop {
        let n = from.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        n_total += n as u64;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok((n_total, h.finalize().to_hex().to_string()))
}

/// Unpacks into `tree` and returns the file list (sorted, hashed).
fn unpack(file: &Path, tree: &Path, name: &str, o: &Opts) -> R<Vec<FileEntry>> {
    let is_zip = std::fs::File::open(file)
        .and_then(|mut f| {
            let mut sig = [0u8; 4];
            f.read_exact(&mut sig).map(|_| sig == *b"PK\x03\x04")
        })
        .unwrap_or(false);
    let mut out = vec![];
    if !(o.unpack && is_zip) {
        let name = name.split('?').next().unwrap_or(name);
        validate_rel_path(name).map_err(|e| e.to_string())?;
        let (size, hash) = copy_hashed(std::fs::File::open(file).map_err(|e| e.to_string())?, &tree.join(name))?;
        out.push(FileEntry { path: name.into(), size, hash, exec: name.to_lowercase().ends_with(".exe"), chunks: vec![] });
        return Ok(out);
    }
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(file).map_err(|e| e.to_string())?))
        .map_err(|e| format!("not a valid zip: {e}"))?;
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().and_then(|e| e.enclosed_name()).map(|p| p.to_string_lossy().replace('\\', "/")))
        .collect();
    // A single top-level folder (GitHub source zips always have one) is dropped when strip_root is on.
    let root = names.first().and_then(|n| n.split('/').next()).map(|r| format!("{r}/"));
    let strip = match &root {
        Some(r) if o.strip_root && names.iter().all(|n| n.starts_with(r.as_str()) || n == r.trim_end_matches('/')) => r.clone(),
        _ => String::new(),
    };
    let sub = o.sub.as_ref().map(|s| format!("{}/", s.trim_matches('/')));
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|e| e.to_string())?;
        if e.is_dir() {
            continue;
        }
        let Some(full) = e.enclosed_name().map(|p| p.to_string_lossy().replace('\\', "/")) else {
            return Err("unsafe path in archive".into());
        };
        let mut rel = full.strip_prefix(&strip).unwrap_or(&full).to_string();
        if let Some(s) = &sub {
            match rel.strip_prefix(s.as_str()) {
                Some(r) => rel = r.to_string(),
                None => continue,
            }
        }
        if rel.is_empty() || o.exclude.iter().any(|g| glob_match(g, &rel)) {
            continue;
        }
        validate_rel_path(&rel).map_err(|e| e.to_string())?;
        let (size, hash) = copy_hashed(&mut e, &tree.join(&rel))?;
        let exec = rel.to_lowercase().ends_with(".exe");
        out.push(FileEntry { path: rel, size, hash, exec, chunks: vec![] });
    }
    out.sort_by_key(|f| f.path.to_lowercase());
    Ok(out)
}

/// Moves changed files from the staging tree into the install folder. Returns how many files were written.
fn apply(new: &Manifest, install: &Path, tree: &Path, deep: bool) -> R<usize> {
    let old = library::read_manifest(install);
    let old_by_path: HashMap<String, &FileEntry> =
        old.as_ref().map(|m| m.files.iter().map(|f| (f.path.to_lowercase(), f)).collect()).unwrap_or_default();
    let kept = |p: &str| new.keep.iter().any(|g| glob_match(g, p));
    let mut written = 0;
    for f in &new.files {
        let target = install.join(&f.path);
        let staged: PathBuf = tree.join(&f.path);
        let exists = target.is_file();
        if kept(&f.path) && exists {
            continue;
        }
        let same = exists
            && std::fs::metadata(&target).map(|m| m.len() == f.size).unwrap_or(false)
            && if deep {
                crate::engine::hash_file(&target).map(|h| h == f.hash).unwrap_or(false)
            } else {
                old_by_path.get(&f.path.to_lowercase()).is_some_and(|o| o.hash == f.hash)
            };
        if same {
            continue;
        }
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&staged, &target).map_err(|e| format!("can't replace {} ({e}). Is it still running?", f.path))?;
        written += 1;
    }
    if let Some(old) = &old {
        let now: std::collections::HashSet<String> = new.files.iter().map(|f| f.path.to_lowercase()).collect();
        for f in &old.files {
            if !now.contains(&f.path.to_lowercase()) && !kept(&f.path) && validate_rel_path(&f.path).is_ok() {
                let _ = std::fs::remove_file(install.join(&f.path));
                crate::engine::remove_empty_parents(install, &f.path);
            }
        }
    }
    Ok(written)
}
