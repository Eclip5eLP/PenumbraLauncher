//! Settings + playtime sync through the Penumbra Account data store (userbase `api/data.php`, app 000007).
//! Both values are small JSON documents (well below the 64 KB per-value limit).
//!
//! - `settings`: `{ updatedAt, values }` with only the settings marked "sync" in docs/07-settings.md. The newer
//!   `updatedAt` wins (whole document).
//! - `playtime`: `{ <item>: { <device id>: seconds } }`. Each PC only writes its own entry, so nothing is lost
//!   when two PCs play at the same time. The UI shows the sum.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::auth::ACCOUNTS;
use crate::paths;
use crate::state::AppState;

/// Settings that follow the account (machine-specific ones like folders, scale, window effects don't).
pub const SYNC_KEYS: &[&str] = &[
    "language", "closeButton", "onGameLaunch", "notifications", "autoUpdateGames", "maxConcurrentJobs",
    "connectionsPerJob", "pauseWhileRunning", "resumeOnStart", "verifyAfterInstall", "createDesktopShortcut",
    "backgroundVideo", "reduceMotion", "useGameAccent", "railOrder", "hiddenItems", "pinnedItems", "cloudSaves",
    "shareTokenWithGames",
];

type Playtime = BTreeMap<String, BTreeMap<String, u64>>;

static PUSH_GEN: AtomicU64 = AtomicU64::new(0);

/// Stable random id of this PC (for per-device playtime).
pub fn device_id() -> String {
    let f = paths::config_dir().join("device.json");
    if let Ok(s) = std::fs::read_to_string(&f)
        && let Ok(v) = serde_json::from_str::<Value>(&s)
        && let Some(id) = v["id"].as_str()
    {
        return id.to_string();
    }
    let id = penumbra_core::chunk::hash_hex(format!("{:?}{:?}", std::time::SystemTime::now(), std::process::id()).as_bytes())[..16].to_string();
    let _ = paths::write_atomic(&f, json!({ "id": id }).to_string().as_bytes());
    id
}

async fn get(app: &AppHandle, key: &str) -> Option<Value> {
    let state = app.state::<AppState>();
    let tok = state.auth.access_token(app).await?;
    let r = state.http.get(format!("{ACCOUNTS}api/data.php?key={key}")).bearer_auth(tok).send().await.ok()?;
    if !r.status().is_success() {
        return None; // 404 = never stored
    }
    serde_json::from_slice(&r.bytes().await.ok()?).ok()
}

async fn put(app: &AppHandle, key: &str, v: &Value) -> Result<(), String> {
    let state = app.state::<AppState>();
    let tok = state.auth.access_token(app).await.ok_or("not signed in")?;
    let r = state
        .http
        .put(format!("{ACCOUNTS}api/data.php?key={key}"))
        .bearer_auth(tok)
        .header("Content-Type", "application/json")
        .body(v.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if r.status().is_success() { Ok(()) } else { Err(format!("account storage answered {}", r.status())) }
}

/// After sign-in and at startup: pull settings (if newer) and playtime.
pub async fn pull_all(app: AppHandle) {
    if !app.state::<AppState>().auth.state().signed_in {
        return;
    }
    pull_settings(&app).await;
    pull_playtime(&app).await;
}

async fn pull_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().sync_settings {
        return;
    }
    let Some(remote) = get(app, "settings").await else {
        push_settings_now(app).await; // first PC: seed the account with this PC's settings
        return;
    };
    let remote_at = remote["updatedAt"].as_u64().unwrap_or(0);
    let local_at = state.settings.lock().unwrap().updated_at;
    if remote_at > local_at {
        let mut patch = serde_json::Map::new();
        if let Some(vals) = remote["values"].as_object() {
            for k in SYNC_KEYS {
                if let Some(v) = vals.get(*k) {
                    patch.insert((*k).into(), v.clone());
                }
            }
        }
        let next = {
            let mut s = state.settings.lock().unwrap();
            match s.patched(&Value::Object(patch)) {
                Ok(mut n) => {
                    n.updated_at = remote_at;
                    let _ = n.save();
                    *s = n.clone();
                    Some(n)
                }
                Err(e) => {
                    tracing::warn!("synced settings rejected: {e}");
                    None
                }
            }
        };
        if let Some(n) = next {
            tracing::info!("settings pulled from account");
            let _ = app.emit("settings:changed", n);
        }
    } else if local_at > remote_at {
        push_settings_now(app).await;
    }
}

async fn push_settings_now(app: &AppHandle) {
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    if !s.sync_settings {
        return;
    }
    let all = serde_json::to_value(&s).unwrap_or_default();
    let values: serde_json::Map<String, Value> =
        SYNC_KEYS.iter().filter_map(|k| all.get(*k).map(|v| ((*k).to_string(), v.clone()))).collect();
    if let Err(e) = put(app, "settings", &json!({ "updatedAt": s.updated_at, "values": values })).await {
        tracing::warn!("settings sync push failed: {e}");
    }
}

/// Called after a settings change that touched synced keys: pushes 5 s later (changes in between are merged).
pub fn schedule_push(app: &AppHandle) {
    let generation = PUSH_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        if PUSH_GEN.load(Ordering::SeqCst) == generation && app.state::<AppState>().auth.state().signed_in {
            push_settings_now(&app).await;
        }
    });
}

/// Seconds played on *other* PCs per item (this PC's own time is in the library).
fn others(pt: &Playtime) -> BTreeMap<String, u64> {
    let me = device_id();
    pt.iter().map(|(item, devs)| (item.clone(), devs.iter().filter(|(d, _)| **d != me).map(|(_, s)| *s).sum())).collect()
}

async fn pull_playtime(app: &AppHandle) {
    let pt: Playtime = get(app, "playtime").await.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let other = others(&pt);
    *app.state::<AppState>().playtime_other.lock().unwrap() = other.clone();
    let _ = app.emit("playtime:changed", other);
}

/// After a play session: write this PC's total for the item (read-modify-write of our own entry only).
pub async fn push_playtime(app: AppHandle, item: String) {
    let state = app.state::<AppState>();
    if !state.auth.state().signed_in {
        return;
    }
    let Some(local) = state.library.lock().unwrap().get(&item).map(|i| i.playtime_sec) else { return };
    let mut pt: Playtime = get(&app, "playtime").await.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    pt.entry(item).or_default().insert(device_id(), local);
    if let Err(e) = put(&app, "playtime", &serde_json::to_value(&pt).unwrap_or_default()).await {
        tracing::warn!("playtime sync failed: {e}");
        return;
    }
    let other = others(&pt);
    *state.playtime_other.lock().unwrap() = other.clone();
    let _ = app.emit("playtime:changed", other);
}
