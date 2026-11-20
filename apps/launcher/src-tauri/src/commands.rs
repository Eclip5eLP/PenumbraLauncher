//! IPC commands (docs/05-launcher-app.md#ipc-contract). Keep them thin: validate, delegate, return.

use std::sync::atomic::Ordering;
use std::time::Duration;

use penumbra_core::catalog::{Catalog, ItemStatus, ItemType};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::jobs::{JobKind, JobView};
use crate::settings::Settings;
use crate::state::{AppState, Library};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    app_version: String,
    settings: Settings,
    catalog: Catalog,
    library: Library,
    jobs: Vec<JobView>,
    running: Vec<String>,
    mock: bool,
}

/// Everything the UI needs for its first paint, in one round trip.
#[tauri::command]
pub fn get_bootstrap(app: AppHandle, state: State<'_, AppState>) -> Bootstrap {
    Bootstrap {
        app_version: app.package_info().version.to_string(),
        settings: state.settings.lock().unwrap().clone(),
        catalog: state.catalog.clone(),
        library: state.library.lock().unwrap().clone(),
        jobs: state.jobs.list(),
        running: state.running.lock().unwrap().iter().cloned().collect(),
        mock: true,
    }
}

/// Called by the UI after its first paint: shows the window and records startup time.
#[tauri::command]
pub fn app_ready(app: AppHandle, state: State<'_, AppState>, first_paint_ms: f64) -> AppResult<u64> {
    let ms = state.started.elapsed().as_millis() as u64;
    show_main(&app, &state)?;
    tracing::info!(startup_ms = ms, webview_first_paint_ms = first_paint_ms, "window shown");
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

#[tauri::command]
pub fn install(state: State<'_, AppState>, item: String, channel: String, path: Option<String>) -> AppResult<JobView> {
    let it = state.item(&item).ok_or_else(|| AppError::UnknownItem(item.clone()))?;
    if it.kind == ItemType::Link || it.status == ItemStatus::ComingSoon {
        return Err(AppError::InvalidState(format!("{} can't be installed", it.name)));
    }
    let ch = it.channels.get(&channel).ok_or_else(|| AppError::InvalidState(format!("no channel {channel}")))?;
    let version = ch.version.clone().ok_or_else(|| AppError::InvalidState("nothing published yet".into()))?;
    let installed = state.library.lock().unwrap().get(&item).cloned();
    let (kind, path) = match installed {
        Some(i) if i.version == version && i.channel == channel => {
            return Err(AppError::InvalidState("already up to date".into()));
        }
        Some(i) => (JobKind::Update, i.path),
        None => (JobKind::Install, path.unwrap_or_else(|| format!(r"D:\Penumbra Games\{}", it.name))),
    };
    // Updates download roughly a third of a full install in the simulation.
    let full = ch.download.unwrap_or(100_000_000);
    let total = if kind == JobKind::Update { full / 3 } else { full };
    state.jobs.enqueue(&item, kind, &channel, &version, &path, total)
}

#[tauri::command]
pub fn pause_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.set_paused(id, true)
}

#[tauri::command]
pub fn resume_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.set_paused(id, false)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.cancel(id)
}

#[tauri::command]
pub fn prioritize_job(state: State<'_, AppState>, id: u64) -> AppResult<()> {
    state.jobs.prioritize(id)
}

#[tauri::command]
pub fn clear_finished_jobs(state: State<'_, AppState>) {
    state.jobs.clear_finished()
}

#[tauri::command]
pub fn uninstall(app: AppHandle, state: State<'_, AppState>, item: String) -> AppResult<()> {
    if state.running.lock().unwrap().contains(&item) {
        return Err(AppError::InvalidState("close it before uninstalling".into()));
    }
    let removed = state.library.lock().unwrap().remove(&item);
    if removed.is_none() {
        return Err(AppError::InvalidState("not installed".into()));
    }
    tracing::info!(%item, "uninstalled (simulated)");
    let _ = app.emit("library:changed", state.library.lock().unwrap().clone());
    Ok(())
}

#[derive(Serialize, Clone)]
struct ProcessEvent {
    item: String,
    running: bool,
}

/// M1: simulates an 8-second play session. Real process launch + Job Object tracking comes in M3.
#[tauri::command]
pub fn launch(app: AppHandle, state: State<'_, AppState>, item: String) -> AppResult<()> {
    if !state.library.lock().unwrap().contains_key(&item) {
        return Err(AppError::InvalidState("not installed".into()));
    }
    if !state.running.lock().unwrap().insert(item.clone()) {
        return Err(AppError::InvalidState("already running".into()));
    }
    tracing::info!(%item, "launch (simulated)");
    let _ = app.emit("process:changed", ProcessEvent { item: item.clone(), running: true });

    let on_launch = state.settings.lock().unwrap().on_game_launch.clone();
    if (on_launch == "minimize" || on_launch == "tray")
        && let Some(w) = app.get_webview_window("main")
    {
        let _ = w.minimize();
    }

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        let state = app2.state::<AppState>();
        state.running.lock().unwrap().remove(&item);
        if let Some(i) = state.library.lock().unwrap().get_mut(&item) {
            i.playtime_sec += 8;
            i.last_played = Some(
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            );
        }
        let _ = app2.emit("process:changed", ProcessEvent { item, running: false });
        let _ = app2.emit("library:changed", state.library.lock().unwrap().clone());
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
