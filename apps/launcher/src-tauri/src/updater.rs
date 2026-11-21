//! Launcher self-update (docs/09-self-update-and-release.md). Checks 10 s after start and every 6 h. With
//! `autoUpdateLauncher` the signed installer is downloaded in the background and the title bar offers
//! "Restart to update"; otherwise it only says an update is available. Installing never happens without a click.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterState {
    /// `idle` | `checking` | `available` | `downloading` | `ready` | `installing` | `error`
    pub state: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    /// 0..=100 while downloading
    pub progress: Option<u8>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Updater {
    pub view: Mutex<UpdaterState>,
    pending: Mutex<Option<(Update, Option<Vec<u8>>)>>,
    busy: tokio::sync::Mutex<()>,
}

fn emit(app: &AppHandle, f: impl FnOnce(&mut UpdaterState)) {
    let up = &app.state::<AppState>().updater;
    let v = {
        let mut v = up.view.lock().unwrap();
        f(&mut v);
        v.clone()
    };
    let _ = app.emit("updater:state", v);
}

fn endpoint(app: &AppHandle) -> String {
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    let file = if s.launcher_channel == "beta" { "latest-beta.json" } else { "latest.json" };
    format!("{}launcher/{file}", s.api_base)
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            let _ = check(app.clone(), false).await;
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

/// Checks for an update; downloads it right away when auto-update is on (or `download` is forced).
pub async fn check(app: AppHandle, download: bool) -> Result<UpdaterState, String> {
    let state = app.state::<AppState>();
    let Ok(_guard) = state.updater.busy.try_lock() else {
        return Ok(state.updater.view.lock().unwrap().clone());
    };
    if state.settings.lock().unwrap().offline_mode {
        return Ok(state.updater.view.lock().unwrap().clone());
    }
    if state.updater.view.lock().unwrap().state == "ready" {
        return Ok(state.updater.view.lock().unwrap().clone());
    }
    emit(&app, |v| {
        v.state = "checking".into();
        v.error = None;
    });
    let url = endpoint(&app).parse().map_err(|e| format!("bad update URL: {e}"))?;
    let result = async {
        let updater = app.updater_builder().endpoints(vec![url]).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
        updater.check().await.map_err(|e| e.to_string())
    }
    .await;
    let update = match result {
        Ok(Some(u)) => u,
        Ok(None) => {
            emit(&app, |v| *v = UpdaterState { state: "idle".into(), ..Default::default() });
            return Ok(state.updater.view.lock().unwrap().clone());
        }
        Err(e) => {
            tracing::warn!("launcher update check failed: {e}");
            emit(&app, |v| {
                v.state = "error".into();
                v.error = Some(e.clone());
            });
            return Err(e);
        }
    };
    tracing::info!(version = %update.version, "launcher update available");
    let (version, notes) = (update.version.clone(), update.body.clone());
    emit(&app, |v| {
        v.state = "available".into();
        v.version = Some(version.clone());
        v.notes = notes.clone();
    });
    let auto = state.settings.lock().unwrap().auto_update_launcher;
    if !(auto || download) {
        *state.updater.pending.lock().unwrap() = Some((update, None));
        return Ok(state.updater.view.lock().unwrap().clone());
    }

    emit(&app, |v| {
        v.state = "downloading".into();
        v.progress = Some(0);
    });
    let mut received = 0u64;
    let app2 = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                received += chunk as u64;
                if let Some(t) = total.filter(|t| *t > 0) {
                    let pct = (received * 100 / t).min(100) as u8;
                    let up = &app2.state::<AppState>().updater;
                    if up.view.lock().unwrap().progress != Some(pct) {
                        emit(&app2, |v| v.progress = Some(pct));
                    }
                }
            },
            || {},
        )
        .await;
    match bytes {
        Ok(b) => {
            *state.updater.pending.lock().unwrap() = Some((update, Some(b)));
            emit(&app, |v| {
                v.state = "ready".into();
                v.progress = None;
            });
            tracing::info!(%version, "launcher update downloaded; waiting for restart");
        }
        Err(e) => {
            tracing::warn!("launcher update download failed: {e}");
            emit(&app, |v| {
                v.state = "error".into();
                v.error = Some(e.to_string());
            });
            return Err(e.to_string());
        }
    }
    Ok(state.updater.view.lock().unwrap().clone())
}

/// Installs the downloaded update (downloads first if needed). The installer replaces the launcher and starts it
/// again; running downloads are paused by the shutdown and resume from their journal afterwards.
pub async fn install(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let has_bytes = state.updater.pending.lock().unwrap().as_ref().is_some_and(|(_, b)| b.is_some());
    if !has_bytes {
        check(app.clone(), true).await?;
    }
    let Some((update, Some(bytes))) = state.updater.pending.lock().unwrap().take() else {
        return Err("no update downloaded".into());
    };
    if !state.running.lock().unwrap().is_empty() {
        *state.updater.pending.lock().unwrap() = Some((update, Some(bytes)));
        return Err("Close running games before updating the launcher.".into());
    }
    emit(&app, |v| v.state = "installing".into());
    tracing::info!(version = %update.version, "installing launcher update");
    // On Windows this starts the (signed, verified) NSIS installer in passive mode and exits the launcher.
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
