//! Install / update / repair engine (docs/05-launcher-app.md#download-engine-details).
//!
//! 1. fetch + verify the signed manifest
//! 2. look at the disk (sizes; full hashes for repair) and plan (penumbra_core::plan)
//! 3. write into `<install>/.penumbra/staging/*.part`: local copies first, then parallel chunk downloads.
//!    `.penumbra/journal.txt` records finished chunks, so a paused/crashed job resumes where it stopped.
//! 4. finalize: hash every staged file, move into place, delete removed files, write install record.
//!
//! Nothing in the install folder is touched before step 4, so an interrupted update leaves a working install.

use std::collections::HashSet;
use std::io::Write;
use std::os::windows::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use penumbra_core::chunk;
use penumbra_core::manifest::Manifest;
use penumbra_core::paths::validate_rel_path;
use penumbra_core::plan::{self, ChunkFetch, LocalState, Plan};
use rayon::prelude::*;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::library::{self, InstallRecord};
use crate::net::path_segment;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Install,
    Update,
    Repair,
}

pub const PHASE_PREPARING: u8 = 0;
pub const PHASE_VERIFYING: u8 = 1;
pub const PHASE_DOWNLOADING: u8 = 2;
pub const PHASE_FINALIZING: u8 = 3;

#[derive(Default)]
pub struct Progress {
    /// Downloaded compressed bytes (or hashed bytes while verifying).
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub phase: AtomicU8,
    /// Set on pause/cancel: blocking writers that are already running stop before touching the disk again.
    pub stop: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub item: String,
    pub kind: Kind,
    pub channel: String,
    pub version: String,
    pub path: PathBuf,
}

pub struct Outcome {
    pub size: u64,
    pub written: usize,
    pub note: Option<String>,
    /// The version actually installed, when it differs from the one queued (GitHub channels move).
    pub version: Option<String>,
}

type R<T> = Result<T, String>;

fn io<E: std::fmt::Display>(ctx: &str) -> impl Fn(E) -> String + '_ {
    move |e| format!("{ctx}: {e}")
}

pub async fn run(app: AppHandle, spec: Spec, prog: Arc<Progress>) -> R<Outcome> {
    let state = app.state::<AppState>();
    if let Some(item) = state.item(&spec.item)
        && matches!(item.source, Some(penumbra_core::catalog::Source::Github { .. } | penumbra_core::catalog::Source::Url { .. }))
    {
        return crate::archive::run(app.clone(), spec, item, prog).await;
    }
    prog.phase.store(PHASE_PREPARING, Ordering::Relaxed);
    let (manifest, manifest_bytes) = fetch_manifest(&app, &state, &spec.item, &spec.version).await?;
    let install = spec.path.clone();
    let meta = library::meta_dir(&install);
    std::fs::create_dir_all(&meta).map_err(io("creating install folder"))?;

    // Old manifest: what's installed now (for repair of the same version this is the new one).
    let old = library::read_manifest(&install);
    let deep = spec.kind == Kind::Repair;
    let plan = {
        let (new, old2, install2, prog2) = (manifest.clone(), old.clone(), install.clone(), prog.clone());
        tokio::task::spawn_blocking(move || survey_and_plan(&new, old2.as_ref(), &install2, deep, &prog2))
            .await
            .map_err(|e| e.to_string())??
    };
    tracing::info!(
        item = %spec.item, version = %spec.version, writes = plan.write.len(), chunks = plan.fetch.len(),
        download = plan.download_bytes, reuse = plan.reuse_bytes, delete = plan.delete.len(), "plan"
    );

    check_space(&install, &plan)?;
    let staging = meta.join("staging");
    std::fs::create_dir_all(&staging).map_err(io("creating staging folder"))?;
    let journal = Journal::open(&meta, &spec.version)?;

    // Local copies (fast, once per job).
    if !journal.has("reuse") && !plan.reuse.is_empty() {
        let (p, inst, st, pr) = (plan.clone(), install.clone(), staging.clone(), prog.clone());
        tokio::task::spawn_blocking(move || copy_local(&p, &inst, &st, &pr)).await.map_err(|e| e.to_string())??;
        journal.add("reuse")?;
    }

    // Downloads.
    let pending: Vec<ChunkFetch> = plan.fetch.iter().filter(|c| !journal.has(&c.hash)).cloned().collect();
    let already: u64 = plan.download_bytes - pending.iter().map(|c| c.csize).sum::<u64>();
    prog.total.store(plan.download_bytes, Ordering::Relaxed);
    prog.done.store(already, Ordering::Relaxed);
    prog.phase.store(PHASE_DOWNLOADING, Ordering::Relaxed);
    let conns = state.settings.lock().unwrap().connections_per_job.clamp(1, 32) as usize;
    let sem = Arc::new(Semaphore::new(conns));
    let journal = Arc::new(journal);
    let plan = Arc::new(plan);
    let mut set = JoinSet::new();
    for c in pending {
        let permit = sem.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let (app2, prog2, staging2, journal2, plan2) =
            (app.clone(), prog.clone(), staging.clone(), journal.clone(), plan.clone());
        set.spawn(async move {
            let _permit = permit;
            fetch_and_write(&app2, &c, &plan2, &staging2, &prog2).await?;
            journal2.add(&c.hash)
        });
        while let Some(r) = set.try_join_next() {
            r.map_err(|e| e.to_string())??;
        }
    }
    while let Some(r) = set.join_next().await {
        r.map_err(|e| e.to_string())??;
    }

    // Finalize.
    prog.phase.store(PHASE_FINALIZING, Ordering::Relaxed);
    let written = plan.write.len();
    let (p, inst, st, m) = (plan.clone(), install.clone(), staging.clone(), manifest.clone());
    tokio::task::spawn_blocking(move || finalize(&p, &m, &inst, &st)).await.map_err(|e| e.to_string())??;
    std::fs::write(meta.join("manifest.json"), &manifest_bytes).map_err(io("writing manifest"))?;
    let record = InstallRecord { item: spec.item.clone(), version: spec.version.clone(), channel: spec.channel.clone() };
    let _ = std::fs::write(meta.join("install.json"), serde_json::to_vec_pretty(&record).unwrap_or_default());
    journal.remove();
    let _ = std::fs::remove_dir_all(&staging);

    let note = match spec.kind {
        Kind::Repair if written == 0 => Some("All files are OK.".to_string()),
        Kind::Repair => Some(format!("Repaired {written} file{}.", if written == 1 { "" } else { "s" })),
        _ => None,
    };
    Ok(Outcome { size: manifest.total_size, written, note, version: None })
}

/// Removes everything a cancelled job left behind. A cancelled fresh install also removes the folder it created.
pub fn discard(install: &Path, fresh: bool) {
    // A write that was already running may hold a file open for a moment: retry briefly.
    for wait_ms in [300u64, 1000, 3000] {
        std::thread::sleep(Duration::from_millis(wait_ms));
        discard_once(install, fresh);
        if !library::meta_dir(install).join("staging").exists() {
            break;
        }
    }
}

fn discard_once(install: &Path, fresh: bool) {
    let meta = library::meta_dir(install);
    let _ = std::fs::remove_dir_all(meta.join("staging"));
    let _ = std::fs::remove_dir_all(meta.join("staging-tree"));
    let _ = std::fs::remove_file(meta.join("journal.txt"));
    if fresh && !meta.join("manifest.json").exists() {
        let _ = std::fs::remove_dir(&meta);
        let _ = std::fs::remove_dir(install); // only succeeds when empty
    }
}

// ---- manifest -------------------------------------------------------------------------------------------

async fn get(state: &AppState, url: &str) -> R<reqwest::Response> {
    let mut last = String::new();
    for attempt in 0..4u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 << attempt)).await;
        }
        match state.http.get(url).send().await {
            Ok(r) if r.status().is_success() || r.status().is_client_error() => return Ok(r),
            Ok(r) => last = format!("server answered {}", r.status()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("download failed ({last})"))
}

async fn fetch_manifest(app: &AppHandle, state: &AppState, item: &str, version: &str) -> R<(Manifest, Vec<u8>)> {
    let base = state.api_base();
    let v = path_segment(version);
    // Public manifests come from the CDN; gated ones only through the API.
    let (zst, sig) = {
        let r = get(state, &format!("{base}content/{item}/{v}.json.zst")).await?;
        if r.status().is_success() {
            let zst = r.bytes().await.map_err(io("manifest"))?;
            let s = get(state, &format!("{base}content/{item}/{v}.json.sig")).await?;
            if !s.status().is_success() {
                return Err(format!("manifest signature missing ({})", s.status()));
            }
            (zst.to_vec(), s.text().await.map_err(io("signature"))?)
        } else {
            let url = format!("{base}api/v1/manifest.php?item={item}&version={v}");
            let mut req = state.http.get(&url);
            if let Some(tok) = state.auth.access_token(app).await {
                req = req.bearer_auth(tok);
            }
            let r = req.send().await.map_err(|e| format!("download failed ({e})"))?;
            match r.status().as_u16() {
                200 => {}
                401 => return Err("Sign in to download this.".into()),
                403 => return Err("Your account doesn't have access to this.".into()),
                404 => return Err(format!("Version {version} is not on the server.")),
                s => return Err(format!("server answered {s}")),
            }
            let sig = r.headers().get("x-penumbra-sig").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
            (r.bytes().await.map_err(io("manifest"))?.to_vec(), sig)
        }
    };
    let json = zstd::bulk::decompress(&zst, 256 << 20).map_err(io("manifest decompress"))?;
    penumbra_core::sign::verify_bytes(&crate::catalog::public_key(), &json, &sig)
        .map_err(|e| format!("manifest rejected: {e}"))?;
    let m: Manifest = serde_json::from_slice(&json).map_err(io("manifest format"))?;
    m.validate().map_err(io("manifest invalid"))?;
    if m.item != item || m.version != version {
        return Err("manifest belongs to a different item/version".into());
    }
    Ok((m, json))
}

// ---- survey + plan --------------------------------------------------------------------------------------

fn survey_and_plan(new: &Manifest, old: Option<&Manifest>, install: &Path, deep: bool, prog: &Progress) -> R<Plan> {
    let mut paths: Vec<(&str, Option<&str>)> = new.files.iter().map(|f| (f.path.as_str(), Some(f.hash.as_str()))).collect();
    if let Some(o) = old {
        let known: HashSet<String> = new.files.iter().map(|f| f.path.to_lowercase()).collect();
        paths.extend(o.files.iter().filter(|f| !known.contains(&f.path.to_lowercase())).map(|f| (f.path.as_str(), None)));
    }
    if deep {
        prog.phase.store(PHASE_VERIFYING, Ordering::Relaxed);
        let total: u64 = paths
            .iter()
            .filter_map(|(p, _)| std::fs::metadata(install.join(p)).ok().map(|m| m.len()))
            .sum();
        prog.total.store(total, Ordering::Relaxed);
        prog.done.store(0, Ordering::Relaxed);
    }
    let states: std::collections::HashMap<String, LocalState> = paths
        .par_iter()
        .map(|(p, expected)| {
            let full = install.join(p);
            let st = match std::fs::metadata(&full) {
                Ok(m) if m.is_file() => {
                    let verified = match (deep, expected) {
                        (true, Some(h)) => {
                            let ok = hash_file(&full).map(|x| x == *h).unwrap_or(false);
                            prog.done.fetch_add(m.len(), Ordering::Relaxed);
                            Some(ok)
                        }
                        _ => None,
                    };
                    LocalState::Present { size: m.len(), verified }
                }
                _ => LocalState::Missing,
            };
            (p.to_lowercase(), st)
        })
        .collect();
    let lookup = |p: &str| states.get(&p.to_lowercase()).copied().unwrap_or(LocalState::Missing);
    Ok(plan::plan(new, old, &lookup))
}

pub fn hash_file(p: &Path) -> std::io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(p)?))?;
    Ok(h.finalize().to_hex().to_string())
}

fn check_space(install: &Path, plan: &Plan) -> R<()> {
    let needed: u64 = plan.write.iter().map(|w| w.size).sum();
    if let Some(free) = crate::sys::free_space(install)
        && free < needed + (64 << 20)
    {
        return Err(format!(
            "Not enough disk space: {} needed, {} free.",
            crate::sys::human(needed),
            crate::sys::human(free)
        ));
    }
    Ok(())
}

// ---- staging --------------------------------------------------------------------------------------------

fn staging_file(staging: &Path, rel: &str) -> PathBuf {
    staging.join(format!("{}.part", &chunk::hash_hex(rel.to_lowercase().as_bytes())[..24]))
}

fn write_at(path: &Path, offset: u64, data: &[u8]) -> R<()> {
    let f = std::fs::OpenOptions::new().write(true).create(true).truncate(false).open(path).map_err(io("staging"))?;
    let mut done = 0;
    while done < data.len() {
        let n = f.seek_write(&data[done..], offset + done as u64).map_err(io("writing"))?;
        done += n;
    }
    Ok(())
}

fn copy_local(plan: &Plan, install: &Path, staging: &Path, prog: &Progress) -> R<()> {
    for c in &plan.reuse {
        if prog.stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        let src = std::fs::File::open(install.join(&c.src_path)).map_err(io("reading existing file"))?;
        let mut buf = vec![0u8; c.size as usize];
        // seek_read may return short reads; loop until full.
        let mut got = 0usize;
        while got < buf.len() {
            let n = src.seek_read(&mut buf[got..], c.src_offset + got as u64).map_err(io("reading existing file"))?;
            if n == 0 {
                return Err(format!("{} is shorter than expected", c.src_path));
            }
            got += n;
        }
        write_at(&staging_file(staging, &plan.write[c.write].path), c.file_offset, &buf)?;
    }
    Ok(())
}

async fn fetch_and_write(app: &AppHandle, c: &ChunkFetch, plan: &Plan, staging: &Path, prog: &Arc<Progress>) -> R<()> {
    let state = app.state::<AppState>();
    let url = format!("{}{}", state.api_base(), chunk::chunk_path(&c.hash));
    let mut last = String::new();
    for attempt in 0..5u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 << attempt)).await;
        }
        let mut counted = 0u64;
        let result: R<Vec<u8>> = async {
            let mut resp = state.http.get(&url).send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("server answered {}", resp.status()));
            }
            let mut buf = Vec::with_capacity(c.csize as usize);
            while let Some(piece) = resp.chunk().await.map_err(|e| e.to_string())? {
                wait_while_game_runs(&state).await;
                let limit = state.settings.lock().unwrap().bandwidth_limit;
                state.limiter.take(piece.len(), limit).await;
                buf.extend_from_slice(&piece);
                prog.done.fetch_add(piece.len() as u64, Ordering::Relaxed);
                counted += piece.len() as u64;
            }
            Ok(buf)
        }
        .await;
        let stored = match result {
            Ok(b) => b,
            Err(e) => {
                prog.done.fetch_sub(counted, Ordering::Relaxed);
                last = e;
                continue;
            }
        };
        // Decode + verify + write off the async runtime.
        let (hash, len, targets) = (c.hash.clone(), c.len, c.targets.clone());
        let files: Vec<PathBuf> = targets.iter().map(|t| staging_file(staging, &plan.write[t.write].path)).collect();
        let stop = prog.clone();
        let r = tokio::task::spawn_blocking(move || -> R<()> {
            let data = chunk::decode_chunk(&stored, &hash, len).map_err(|e| e.to_string())?;
            if stop.stop.load(Ordering::Relaxed) {
                return Err("stopped".into());
            }
            for (t, f) in targets.iter().zip(files) {
                let seg = &data[t.chunk_offset as usize..(t.chunk_offset + t.size) as usize];
                write_at(&f, t.file_offset, seg)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
        match r {
            Ok(()) => return Ok(()),
            Err(e) => {
                prog.done.fetch_sub(counted, Ordering::Relaxed);
                last = e;
            }
        }
    }
    Err(format!("chunk {} failed after 5 attempts: {last}", &c.hash[..12]))
}

pub async fn wait_while_game_runs(state: &AppState) {
    loop {
        let pause = state.settings.lock().unwrap().pause_while_running && !state.running.lock().unwrap().is_empty();
        if !pause {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn finalize(plan: &Plan, manifest: &Manifest, install: &Path, staging: &Path) -> R<()> {
    // Every staged file must hash to its manifest value before anything is replaced.
    plan.write.par_iter().try_for_each(|w| -> R<()> {
        let f = staging_file(staging, &w.path);
        if w.size == 0 {
            std::fs::write(&f, b"").map_err(io("staging"))?;
        }
        let got = hash_file(&f).map_err(io(&w.path))?;
        if got != w.hash {
            let _ = std::fs::remove_file(&f);
            let _ = std::fs::remove_file(f.parent().unwrap().parent().unwrap().join("journal.txt"));
            return Err(format!("{} failed verification; please try again", w.path));
        }
        Ok(())
    })?;
    for w in &plan.write {
        validate_rel_path(&w.path).map_err(|e| e.to_string())?;
        let target = install.join(&w.path);
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(io("creating folders"))?;
        }
        if let Ok(m) = std::fs::metadata(&target)
            && m.permissions().readonly()
        {
            let mut p = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            let _ = std::fs::set_permissions(&target, p);
        }
        std::fs::rename(staging_file(staging, &w.path), &target).map_err(|e| {
            format!("can't replace {} ({e}). Is the game still running?", w.path)
        })?;
    }
    for rel in &plan.delete {
        if validate_rel_path(rel).is_ok() {
            let _ = std::fs::remove_file(install.join(rel));
            remove_empty_parents(install, rel);
        }
    }
    for d in &manifest.dirs {
        if validate_rel_path(d).is_ok() {
            let _ = std::fs::create_dir_all(install.join(d));
        }
    }
    Ok(())
}

pub fn remove_empty_parents(install: &Path, rel: &str) {
    let mut p = install.join(rel);
    while let Some(parent) = p.parent() {
        if parent == install || std::fs::remove_dir(parent).is_err() {
            break;
        }
        p = parent.to_path_buf();
    }
}

// ---- journal --------------------------------------------------------------------------------------------

struct Journal {
    path: PathBuf,
    done: Mutex<HashSet<String>>,
    file: Mutex<std::fs::File>,
}

impl Journal {
    fn open(meta: &Path, version: &str) -> R<Journal> {
        let path = meta.join("journal.txt");
        let header = format!("version {version}");
        let mut done = HashSet::new();
        let mut valid = false;
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut lines = text.lines();
            if lines.next() == Some(header.as_str()) {
                valid = true;
                done.extend(lines.map(String::from));
            }
        }
        if !valid {
            // Different (or no) job before: start clean.
            let _ = std::fs::remove_dir_all(meta.join("staging"));
            let _ = std::fs::create_dir_all(meta.join("staging"));
            std::fs::write(&path, format!("{header}\n")).map_err(io("journal"))?;
        } else if !done.is_empty() {
            tracing::info!(entries = done.len(), "resuming from journal");
        }
        let file = std::fs::OpenOptions::new().append(true).open(&path).map_err(io("journal"))?;
        Ok(Journal { path, done: Mutex::new(done), file: Mutex::new(file) })
    }

    fn has(&self, key: &str) -> bool {
        self.done.lock().unwrap().contains(key)
    }

    fn add(&self, key: &str) -> R<()> {
        writeln!(self.file.lock().unwrap(), "{key}").map_err(io("journal"))?;
        self.done.lock().unwrap().insert(key.to_string());
        Ok(())
    }

    fn remove(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
