//! IPC commands (docs/05-launcher-app.md#ipc-contract). Keep them thin: validate, delegate, return.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use penumbra_core::catalog::{Catalog, ItemStatus, ItemType};
use penumbra_core::paths::validate_rel_path;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::catalog::CatalogStatus;
use crate::error::{AppError, AppResult};
use crate::jobs::{JobKind, JobView};
use crate::library::{self, Installed, Library, now_unix};
use crate::settings::Settings;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    app_version: String,
    settings: Settings,
    catalog: Catalog,
    catalog_status: CatalogStatus,
    library: Library,
    jobs: Vec<JobView>,
    running: Vec<String>,
    suggested_library: String,
    mock: bool,
}

/// Everything the UI needs for its first paint, in one round trip (cached catalog; refresh follows).
#[tauri::command]
pub fn get_bootstrap(app: AppHandle, state: State<'_, AppState>) -> Bootstrap {
    let settings = state.settings.lock().unwrap().clone();
    let suggested = settings
        .default_library
        .clone()
        .unwrap_or_else(|| crate::sys::suggested_library().to_string_lossy().into_owned());
    Bootstrap {
        app_version: app.package_info().version.to_string(),
        settings,
        catalog: state.catalog.read().unwrap().clone(),
        catalog_status: state.catalog_status.lock().unwrap().clone(),
        library: state.library.lock().unwrap().clone(),
        jobs: state.jobs.list(),
        running: state.running.lock().unwrap().keys().cloned().collect(),
        suggested_library: suggested,
        mock: false,
    }
}

/// Called by the UI after its first paint: shows the window, then refreshes the catalog in the background.
#[tauri::command]
pub fn app_ready(app: AppHandle, state: State<'_, AppState>, first_paint_ms: f64) -> AppResult<u64> {
    let ms = state.started.elapsed().as_millis() as u64;
    show_main(&app, &state)?;
    tracing::info!(startup_ms = ms, webview_first_paint_ms = first_paint_ms, "window shown");
    tauri::async_runtime::spawn(crate::catalog::refresh(app.clone()));
    Ok(ms)
}

pub fn show_main(app: &AppHandle, state: &AppState) -> AppResult<()> {
    if state.shown.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    if let Some(w) = app.get_webview_window("main") {
        w.show()?;
        w.set_focus()?;
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_catalog(app: AppHandle) {
    crate::catalog::refresh(app).await
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<'_, AppState>, patch: Value) -> AppResult<Settings> {
    let next = {
        let mut s = state.settings.lock().unwrap();
        let next = s.patched(&patch)?;
        next.save()?;
        *s = next.clone();
        next
    };
    let _ = app.emit("settings:changed", &next);
    Ok(next)
}

// ---- install / update / repair ---------------------------------------------------------------------------

fn installable(state: &AppState, item: &str, channel: &str) -> AppResult<(penumbra_core::catalog::Item, String)> {
    let it = state.item(item).ok_or_else(|| AppError::UnknownItem(item.into()))?;
    if it.kind == ItemType::Link || it.status == ItemStatus::ComingSoon {
        return Err(AppError::InvalidState(format!("{} can't be installed", it.name)));
    }
    if !matches!(it.source, None | Some(penumbra_core::catalog::Source::Penumbra)) {
        return Err(AppError::InvalidState("GitHub downloads arrive in the next update (M4).".into()));
    }
    let version = it
        .channels
        .get(channel)
        .and_then(|c| c.version.clone())
        .ok_or_else(|| AppError::InvalidState("nothing published on this channel yet".into()))?;
    Ok((it, version))
}

/// An install folder must be absolute, below a drive root, and empty (or a previous install of this item).
fn check_install_dir(path: &Path, item: &str) -> AppResult<()> {
    if !path.is_absolute() || path.parent().is_none_or(|p| p.parent().is_none()) {
        return Err(AppError::InvalidState("Choose a folder (not a drive root).".into()));
    }
    if path.exists() {
        if !path.is_dir() {
            return Err(AppError::InvalidState("That path is a file.".into()));
        }
        let ours = library::read_record(path).is_some_and(|r| r.item == item);
        let empty = std::fs::read_dir(path).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !ours && !empty {
            return Err(AppError::InvalidState("That folder isn't empty. Pick an empty or new folder.".into()));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn install(state: State<'_, AppState>, item: String, channel: String, path: Option<String>) -> AppResult<JobView> {
    let (it, version) = installable(&state, &item, &channel)?;
    let installed = state.library.lock().unwrap().get(&item).cloned();
    match installed {
        Some(i) if i.version == version && i.channel == channel => {
            Err(AppError::InvalidState("already up to date".into()))
        }
        Some(i) => state.jobs.enqueue(&item, JobKind::Update, &channel, &version, &i.path),
        None => {
            let path = match path {
                Some(p) => PathBuf::from(p.trim()),
                None => crate::sys::suggested_library().join(crate::sys::folder_name(&it.name)),
            };
            check_install_dir(&path, &item)?;
            state.jobs.enqueue(&item, JobKind::Install, &channel, &version, &path.to_string_lossy())
        }
    }
}

#[tauri::command]
pub fn verify(state: State<'_, AppState>, item: String) -> AppResult<JobView> {
    let inst = state.library.lock().unwrap().get(&item).cloned().ok_or_else(|| AppError::InvalidState("not installed".into()))?;
    if state.running.lock().unwrap().contains_key(&item) {
        return Err(AppError::InvalidState("Close it before verifying.".into()));
    }
    state.jobs.enqueue(&item, JobKind::Repair, &inst.channel, &inst.version, &inst.path)
}

#[tauri::command]
pub fn pause_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.pause(id)
}

#[tauri::command]
pub fn resume_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.resume(id)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.cancel(id)
}

#[tauri::command]
pub fn retry_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.retry(id)
}

#[tauri::command]
pub fn prioritize_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.prioritize(id)
}

#[tauri::command]
pub fn clear_finished_jobs(state: State<'_, AppState>) {
    state.jobs.clear_finished()
}

// ---- uninstall / locate / folders ------------------------------------------------------------------------

#[tauri::command]
pub async fn uninstall(app: AppHandle, item: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().contains_key(&item) {
        return Err(AppError::InvalidState("Close it before uninstalling.".into()));
    }
    if state.jobs.has_active_job(&item) {
        return Err(AppError::InvalidState("Cancel its download first.".into()));
    }
    let inst = state.library.lock().unwrap().get(&item).cloned().ok_or_else(|| AppError::InvalidState("not installed".into()))?;
    let root = PathBuf::from(&inst.path);
    tauri::async_runtime::spawn_blocking(move || remove_install(&root))
        .await
        .map_err(|e| AppError::InvalidState(e.to_string()))??;
    tracing::info!(%item, path = %inst.path, "uninstalled");
    let lib = state.update_library(|lib| {
        lib.remove(&item);
    });
    let _ = app.emit("library:changed", lib);
    Ok(())
}

/// Deletes only what the manifest lists (plus our metadata), then empty folders. Files the user or the game
/// created that the manifest doesn't know stay where they are.
fn remove_install(root: &Path) -> AppResult<()> {
    if let Some(m) = library::read_manifest(root) {
        for f in &m.files {
            if validate_rel_path(&f.path).is_ok() {
                let p = root.join(&f.path);
                if let Err(e) = std::fs::remove_file(&p)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    return Err(AppError::InvalidState(format!("can't delete {} ({e})", f.path)));
                }
                crate::engine::remove_empty_parents(root, &f.path);
            }
        }
    }
    let _ = std::fs::remove_dir_all(library::meta_dir(root));
    let _ = std::fs::remove_dir(root);
    Ok(())
}

/// Adds an existing install folder (e.g. after reinstalling Windows) back to the library.
#[tauri::command]
pub fn locate_install(app: AppHandle, state: State<'_, AppState>, item: String, path: String) -> AppResult<()> {
    let root = PathBuf::from(&path);
    let rec = library::read_record(&root).filter(|r| r.item == item);
    let manifest = library::read_manifest(&root);
    let (Some(rec), Some(m)) = (rec, manifest) else {
        return Err(AppError::InvalidState("No Penumbra install of this item in that folder.".into()));
    };
    let lib = state.update_library(|lib| {
        lib.insert(
            item.clone(),
            Installed { version: rec.version, channel: rec.channel, path, size: m.total_size, playtime_sec: 0, last_played: None },
        );
    });
    let _ = app.emit("library:changed", lib);
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file();
    if let Some(s) = start.filter(|s| Path::new(s).exists()) {
        dialog = dialog.set_directory(s);
    }
    tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn disk_space(path: String) -> Option<u64> {
    crate::sys::free_space(Path::new(&path))
}

#[tauri::command]
pub fn open_folder(app: AppHandle, state: State<'_, AppState>, item: String) -> AppResult<()> {
    let inst = state.library.lock().unwrap().get(&item).cloned().ok_or_else(|| AppError::InvalidState("not installed".into()))?;
    app.opener().open_path(inst.path, None::<&str>).map_err(|e| AppError::InvalidState(e.to_string()))
}

// ---- launch ------------------------------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct ProcessEvent {
    item: String,
    running: bool,
}

#[tauri::command]
pub fn launch(app: AppHandle, state: State<'_, AppState>, item: String) -> AppResult<()> {
    let inst = state.library.lock().unwrap().get(&item).cloned().ok_or_else(|| AppError::InvalidState("not installed".into()))?;
    if state.running.lock().unwrap().contains_key(&item) {
        return Err(AppError::InvalidState("already running".into()));
    }
    if state.jobs.has_active_job(&item) {
        return Err(AppError::InvalidState("wait for the download to finish".into()));
    }
    let root = PathBuf::from(&inst.path);
    let manifest = library::read_manifest(&root)
        .ok_or_else(|| AppError::InvalidState("Install data is missing. Use Verify & repair.".into()))?;
    let launch = manifest.launch.clone().ok_or_else(|| AppError::InvalidState("nothing to launch".into()))?;
    validate_rel_path(&launch.exe).map_err(|e| AppError::InvalidState(e.to_string()))?;
    let exe = root.join(&launch.exe);
    if !exe.is_file() {
        return Err(AppError::InvalidState(format!("{} is missing. Use Verify & repair.", launch.exe)));
    }
    let cwd = match &launch.workdir {
        Some(w) if validate_rel_path(w).is_ok() => root.join(w),
        _ => exe.parent().unwrap_or(&root).to_path_buf(),
    };
    let tracked = crate::process::launch(&exe, &launch.args, &cwd).map_err(|e| AppError::InvalidState(format!("can't start: {e}")))?;
    tracing::info!(%item, exe = %exe.display(), "launched");
    state.running.lock().unwrap().insert(item.clone(), Instant::now());
    let _ = app.emit("process:changed", ProcessEvent { item: item.clone(), running: true });

    let on_launch = state.settings.lock().unwrap().on_game_launch.clone();
    if let Some(w) = app.get_webview_window("main") {
        match on_launch.as_str() {
            "minimize" | "tray" => {
                let _ = w.minimize();
            }
            "close" => app.exit(0),
            _ => {}
        }
    }

    let app2 = app.clone();
    std::thread::spawn(move || {
        tracked.wait();
        let state = app2.state::<AppState>();
        let started = state.running.lock().unwrap().remove(&item);
        let secs = started.map(|s| s.elapsed().as_secs()).unwrap_or(0);
        tracing::info!(%item, secs, "game closed");
        let lib = state.update_library(|lib| {
            if let Some(i) = lib.get_mut(&item) {
                i.playtime_sec += secs;
                i.last_played = Some(now_unix());
            }
        });
        let _ = app2.emit("process:changed", ProcessEvent { item, running: false });
        let _ = app2.emit("library:changed", lib);
    });
    Ok(())
}

/// Opens a URL that belongs to a catalog item (its `url` or one of its `links`). Never arbitrary URLs.
#[tauri::command]
pub fn open_item_link(app: AppHandle, state: State<'_, AppState>, item: String, key: Option<String>) -> AppResult<()> {
    let it = state.item(&item).ok_or_else(|| AppError::UnknownItem(item.clone()))?;
    let url = match key {
        Some(k) => it.links.get(&k).cloned(),
        None => it.url.clone(),
    }
    .ok_or_else(|| AppError::InvalidState("no such link".into()))?;
    if !url.starts_with("https://") {
        return Err(AppError::InvalidState("only https links can be opened".into()));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| AppError::InvalidState(e.to_string()))
}
