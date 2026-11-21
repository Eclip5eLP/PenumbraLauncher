//! Cloud saves (docs/08-accounts-and-cloud.md). Before a game starts, newer cloud saves are downloaded (after a
//! local backup); after it closes, changed saves are uploaded. Snapshots are zips whose entry names start with the
//! root token (`LOCALAPPDATA/Pokemon___Gracidea_Version/save0.ini`), so they restore on any PC.
//!
//! Sync state per item (`%APPDATA%\Penumbra Launcher\cloudsaves\<item>.json`) remembers the revision + snapshot
//! hash last synced. Cloud changed + local unchanged -> download. Local changed + cloud unchanged -> upload after
//! play. Both changed -> conflict, the user picks.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use penumbra_core::catalog::{CloudSaves, Item};
use penumbra_core::paths::validate_rel_path;
use penumbra_core::plan::glob_match;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::paths;
use crate::state::AppState;

type R<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudMeta {
    pub rev: u64,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SyncState {
    rev: u64,
    hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "result")]
pub enum Before {
    /// Nothing to do (or cloud saves off / not applicable).
    UpToDate,
    Downloaded,
    Conflict { cloud: CloudMeta },
}

struct File {
    /// Zip entry name: `<TOKEN>/<rel>`.
    entry: String,
    path: PathBuf,
}

const TOKENS: &[&str] = &["APPDATA", "LOCALAPPDATA", "USERPROFILE", "DOCUMENTS", "INSTALL"];

fn root_for(token: &str, install: &Path) -> Option<PathBuf> {
    let env = |v: &str| std::env::var_os(v).map(PathBuf::from);
    match token {
        "APPDATA" => env("APPDATA"),
        "LOCALAPPDATA" => env("LOCALAPPDATA"),
        "USERPROFILE" => env("USERPROFILE"),
        "DOCUMENTS" => env("USERPROFILE").map(|p| p.join("Documents")),
        "INSTALL" => Some(install.to_path_buf()),
        _ => None,
    }
}

/// `%LOCALAPPDATA%/X/save*.ini` -> ("LOCALAPPDATA", "X/save*.ini")
fn split_pattern(p: &str) -> Option<(String, String)> {
    let p = p.replace('\\', "/");
    let (head, rest) = p.split_once('/')?;
    let token = match head {
        "{install}" => "INSTALL".to_string(),
        h if h.starts_with('%') && h.ends_with('%') && h.len() > 2 => h[1..h.len() - 1].to_ascii_uppercase(),
        _ => return None,
    };
    TOKENS.contains(&token.as_str()).then_some((token, rest.to_string()))
}

/// Finds the save files on this PC.
fn collect(cfg: &CloudSaves, install: &Path) -> Vec<File> {
    let mut out: Vec<File> = vec![];
    for pattern in &cfg.paths {
        let Some((token, glob)) = split_pattern(pattern) else {
            tracing::warn!(%pattern, "ignoring cloud save path with unknown root");
            continue;
        };
        let Some(root) = root_for(&token, install) else { continue };
        // Walk only below the fixed (wildcard-free) part of the glob.
        let fixed: Vec<&str> = glob.split('/').take_while(|s| !s.contains(['*', '?'])).collect();
        let fixed = if fixed.len() == glob.split('/').count() { &fixed[..fixed.len() - 1] } else { &fixed[..] };
        let base = fixed.iter().fold(root.clone(), |p, s| p.join(s));
        if !base.is_dir() {
            continue;
        }
        for e in walk(&base, 8).into_iter().take(10_000) {
            let rel = e.strip_prefix(&root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            if glob_match(&glob, &rel) && !cfg.exclude.iter().any(|x| glob_match(x, &rel)) && validate_rel_path(&rel).is_ok() {
                let entry = format!("{token}/{rel}");
                if !out.iter().any(|f| f.entry.eq_ignore_ascii_case(&entry)) {
                    out.push(File { entry, path: e });
                }
            }
        }
    }
    out.sort_by_key(|f| f.entry.to_lowercase());
    out
}

fn walk(dir: &Path, depth: u32) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() && depth > 0 => out.extend(walk(&p, depth - 1)),
            Ok(t) if t.is_file() => out.push(p),
            _ => {}
        }
    }
    out
}

/// Content fingerprint of a save set (names + file hashes); empty set = "".
fn fingerprint(files: &[File]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut h = blake3::Hasher::new();
    for f in files {
        let fh = crate::engine::hash_file(&f.path).unwrap_or_default();
        h.update(f.entry.to_lowercase().as_bytes());
        h.update(b"\0");
        h.update(fh.as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_hex().to_string()
}

fn zip_files(files: &[File]) -> R<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for f in files {
            let mut data = vec![];
            std::fs::File::open(&f.path).and_then(|mut h| h.read_to_end(&mut data)).map_err(|e| format!("{}: {e}", f.entry))?;
            z.start_file(f.entry.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(&data).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

fn state_file(item: &str) -> PathBuf {
    paths::config_dir().join("cloudsaves").join(format!("{item}.json"))
}
fn read_state(item: &str) -> SyncState {
    std::fs::read(state_file(item)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn write_state(item: &str, rev: u64, hash: &str) {
    let s = SyncState { rev, hash: hash.into() };
    let _ = paths::write_atomic(&state_file(item), &serde_json::to_vec(&s).unwrap_or_default());
}

/// Cloud saves apply to this item right now? (setting on, signed in, item declares save paths)
fn applicable(app: &AppHandle, item: &Item) -> bool {
    let state = app.state::<AppState>();
    item.cloud_saves.is_some() && state.settings.lock().unwrap().cloud_saves && state.auth.state().signed_in
}

fn url(app: &AppHandle, item: &str, extra: &str) -> String {
    format!("{}api/v1/saves.php?item={item}{extra}", app.state::<AppState>().api_base())
}

async fn token(app: &AppHandle) -> R<String> {
    app.state::<AppState>().auth.access_token(app).await.ok_or_else(|| "not signed in".to_string())
}

async fn cloud_meta(app: &AppHandle, item: &str) -> R<Option<CloudMeta>> {
    let r = app
        .state::<AppState>()
        .http
        .get(url(app, item, ""))
        .bearer_auth(token(app).await?)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    match r.status().as_u16() {
        200 => Ok(Some(serde_json::from_slice(&r.bytes().await.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?)),
        404 => Ok(None),
        s => Err(format!("cloud saves: server answered {s}")),
    }
}

fn install_of(app: &AppHandle, item: &str) -> PathBuf {
    app.state::<AppState>().library.lock().unwrap().get(item).map(|i| PathBuf::from(&i.path)).unwrap_or_default()
}

/// Before launch: download newer cloud saves when that's safe, report a conflict otherwise.
pub async fn before_launch(app: &AppHandle, item: &Item) -> R<Before> {
    if !applicable(app, item) {
        return Ok(Before::UpToDate);
    }
    let cfg = item.cloud_saves.clone().unwrap();
    let install = install_of(app, &item.id);
    let Some(cloud) = cloud_meta(app, &item.id).await? else { return Ok(Before::UpToDate) };
    let local = {
        let (c, i) = (cfg.clone(), install.clone());
        tokio::task::spawn_blocking(move || fingerprint(&collect(&c, &i))).await.map_err(|e| e.to_string())?
    };
    let st = read_state(&item.id);
    if cloud.hash == local {
        write_state(&item.id, cloud.rev, &local);
        return Ok(Before::UpToDate);
    }
    if st.rev == cloud.rev {
        return Ok(Before::UpToDate); // only this PC changed: uploaded after playing
    }
    if local.is_empty() || local == st.hash {
        download_and_apply(app, item).await?;
        return Ok(Before::Downloaded);
    }
    Ok(Before::Conflict { cloud })
}

/// Replaces the local saves with the newest cloud snapshot (local ones are backed up first).
pub async fn download_and_apply(app: &AppHandle, item: &Item) -> R<()> {
    let cfg = item.cloud_saves.clone().ok_or("item has no cloud saves")?;
    let r = app
        .state::<AppState>()
        .http
        .get(url(app, &item.id, "&download=1"))
        .bearer_auth(token(app).await?)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("cloud saves: server answered {}", r.status()));
    }
    let rev: u64 = r.headers().get("x-penumbra-rev").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let bytes = r.bytes().await.map_err(|e| e.to_string())?.to_vec();
    let (id, install) = (item.id.clone(), install_of(app, &item.id));
    tokio::task::spawn_blocking(move || -> R<()> {
        let existing = collect(&cfg, &install);
        backup(&id, &existing)?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("bad snapshot: {e}"))?;
        let mut written = std::collections::HashSet::new();
        for i in 0..zip.len() {
            let mut e = zip.by_index(i).map_err(|e| e.to_string())?;
            if e.is_dir() {
                continue;
            }
            let name = e.name().map_err(|e| e.to_string())?.replace('\\', "/");
            let Some((token, rel)) = name.split_once('/') else { continue };
            // Only restore files that match this item's declared save paths (a snapshot can't write elsewhere).
            let allowed = cfg.paths.iter().any(|p| {
                split_pattern(p).is_some_and(|(t, g)| t == token && glob_match(&g, rel))
            });
            if !allowed || validate_rel_path(rel).is_err() {
                tracing::warn!(%name, "skipping snapshot entry outside the declared save paths");
                continue;
            }
            let Some(root) = root_for(token, &install) else { continue };
            let dest = root.join(rel);
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let mut data = vec![];
            e.read_to_end(&mut data).map_err(|e| e.to_string())?;
            std::fs::write(&dest, data).map_err(|e| format!("{name}: {e}"))?;
            written.insert(name.to_lowercase());
        }
        // Save slots deleted on the other PC disappear here too (they're in the backup).
        for f in &existing {
            if !written.contains(&f.entry.to_lowercase()) {
                let _ = std::fs::remove_file(&f.path);
            }
        }
        let after = fingerprint(&collect(&cfg, &install));
        write_state(&id, rev, &after);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    tracing::info!(item = %item.id, rev, "cloud saves restored");
    Ok(())
}

fn backup(item: &str, files: &[File]) -> R<()> {
    if files.is_empty() {
        return Ok(());
    }
    let dir = paths::local_dir().join("save-backups").join(item);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("{}.zip", crate::library::now_unix());
    std::fs::write(dir.join(name), zip_files(files)?).map_err(|e| e.to_string())?;
    let mut all: Vec<PathBuf> = std::fs::read_dir(&dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    all.sort();
    while all.len() > 5 {
        let _ = std::fs::remove_file(all.remove(0));
    }
    Ok(())
}

/// After the game closed: upload when the saves changed. `force` uploads even over a newer cloud revision
/// (the user chose "keep this PC's saves").
pub async fn after_exit(app: &AppHandle, item: &Item, force: bool) -> R<bool> {
    if !applicable(app, item) {
        return Ok(false);
    }
    let cfg = item.cloud_saves.clone().unwrap();
    let install = install_of(app, &item.id);
    let (files_hash, zipped) = tokio::task::spawn_blocking(move || -> R<(String, Vec<u8>)> {
        let files = collect(&cfg, &install);
        if files.is_empty() {
            return Ok((String::new(), vec![]));
        }
        Ok((fingerprint(&files), zip_files(&files)?))
    })
    .await
    .map_err(|e| e.to_string())??;
    let st = read_state(&item.id);
    if files_hash.is_empty() || (files_hash == st.hash && !force) {
        return Ok(false);
    }
    let base = if force { cloud_meta(app, &item.id).await?.map(|m| m.rev).unwrap_or(0) } else { st.rev };
    let device = crate::net::path_segment(&std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into()));
    let r = app
        .state::<AppState>()
        .http
        .put(url(app, &item.id, &format!("&base={base}&hash={files_hash}&device={device}")))
        .bearer_auth(token(app).await?)
        .header("Content-Type", "application/zip")
        .body(zipped)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    match r.status().as_u16() {
        200 => {
            let m: CloudMeta = serde_json::from_slice(&r.bytes().await.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            write_state(&item.id, m.rev, &files_hash);
            tracing::info!(item = %item.id, rev = m.rev, "cloud saves uploaded");
            Ok(true)
        }
        409 => Err("Another PC uploaded newer saves. You'll be asked which to keep next time you play.".into()),
        413 => Err("Save data is too large for cloud saves (50 MB).".into()),
        s => Err(format!("cloud saves upload failed ({s})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_split_into_root_and_glob() {
        assert_eq!(
            split_pattern("%LOCALAPPDATA%/Pokemon___Gracidea_Version/save*.ini"),
            Some(("LOCALAPPDATA".into(), "Pokemon___Gracidea_Version/save*.ini".into()))
        );
        assert_eq!(split_pattern("{install}/saves/**"), Some(("INSTALL".into(), "saves/**".into())));
        assert_eq!(split_pattern("%WINDIR%/x"), None);
        assert_eq!(split_pattern("C:/x/y"), None);
    }

    #[test]
    fn collects_matching_files_and_fingerprints_them() {
        let dir = std::env::temp_dir().join(format!("pgl-saves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("saves/sub")).unwrap();
        std::fs::write(dir.join("saves/save0.ini"), b"a").unwrap();
        std::fs::write(dir.join("saves/sub/save1.ini"), b"b").unwrap();
        std::fs::write(dir.join("saves/notes.txt"), b"c").unwrap();
        let cfg = CloudSaves { paths: vec!["{install}/saves/**/*.ini".into()], exclude: vec!["saves/sub/**".into()] };
        let files = collect(&cfg, &dir);
        assert_eq!(files.iter().map(|f| f.entry.as_str()).collect::<Vec<_>>(), vec!["INSTALL/saves/save0.ini"]);
        let a = fingerprint(&files);
        std::fs::write(dir.join("saves/save0.ini"), b"changed").unwrap();
        assert_ne!(a, fingerprint(&collect(&cfg, &dir)));
        assert_eq!(fingerprint(&[]), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
