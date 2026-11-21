//! Penumbra Account sign-in (docs/08-accounts-and-cloud.md): OAuth device flow against the existing userbase,
//! app id 000007. Refresh tokens are single-use and rotate, so refreshing is serialized and the new refresh token
//! is stored before the new access token is used.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::paths;
use crate::state::AppState;

pub const ACCOUNTS: &str = "https://accounts.eclip5e.com/";
pub const CLIENT_ID: &str = "000007";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub signed_in: bool,
    pub user: Option<User>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
}

#[derive(Default)]
pub struct Auth {
    /// Serializes refreshes; holds the current access token and its expiry.
    access: tokio::sync::Mutex<Option<(String, Instant)>>,
    user: Mutex<Option<User>>,
    /// Cancel flag of the running device flow, if any.
    device: Mutex<Option<std::sync::Arc<AtomicBool>>>,
}

fn account_file() -> std::path::PathBuf {
    paths::config_dir().join("account.json")
}

/// Form-encodes `key=value` pairs (values here are ids and URL-safe tokens, but encode anyway).
fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", crate::net::path_segment(v))).collect::<Vec<_>>().join("&")
}

async fn post(http: &reqwest::Client, endpoint: &str, pairs: &[(&str, &str)]) -> Result<(u16, Value), String> {
    let r = http
        .post(format!("{ACCOUNTS}api/{endpoint}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(pairs))
        .send()
        .await
        .map_err(|e| format!("can't reach the account server ({e})"))?;
    let status = r.status().as_u16();
    let body = r.bytes().await.map_err(|e| e.to_string())?;
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

impl Auth {
    /// Signed-in user from the last session (shown immediately; validated by the first refresh).
    pub fn load(&self) {
        if crate::cred::load().is_none() {
            return;
        }
        if let Ok(bytes) = std::fs::read(account_file())
            && let Ok(u) = serde_json::from_slice::<User>(&bytes)
        {
            *self.user.lock().unwrap() = Some(u);
        }
    }

    pub fn state(&self) -> AuthState {
        let user = self.user.lock().unwrap().clone();
        AuthState { signed_in: user.is_some(), user }
    }

    fn set_user(&self, app: &AppHandle, user: Option<User>) {
        let changed = *self.user.lock().unwrap() != user;
        match &user {
            Some(u) => {
                let _ = paths::write_atomic(&account_file(), &serde_json::to_vec(u).unwrap_or_default());
            }
            None => {
                let _ = std::fs::remove_file(account_file());
            }
        }
        *self.user.lock().unwrap() = user;
        if changed {
            let _ = app.emit("auth:changed", self.state());
            tauri::async_runtime::spawn(crate::sync::pull_all(app.clone()));
            // Personalised catalog (restricted items appear/disappear).
            tauri::async_runtime::spawn(crate::catalog::refresh(app.clone()));
        }
    }

    /// Stores a token response into `slot` (the locked access-token slot, so no other refresh can run
    /// in between). The rotated refresh token is persisted before anything else.
    fn accept_tokens(&self, app: &AppHandle, slot: &mut Option<(String, Instant)>, v: &Value) -> Result<String, String> {
        let access = v["access_token"].as_str().ok_or("no access token in response")?.to_string();
        let refresh = v["refresh_token"].as_str().ok_or("no refresh token in response")?;
        if !crate::cred::save(refresh) {
            return Err("couldn't store the sign-in in Windows Credential Manager".into());
        }
        let ttl = v["expires_in"].as_u64().unwrap_or(3600);
        *slot = Some((access.clone(), Instant::now() + Duration::from_secs(ttl.saturating_sub(60))));
        let user: Option<User> = serde_json::from_value(v["user"].clone()).ok();
        self.set_user(app, user);
        Ok(access)
    }

    /// A valid access token, refreshing when needed. None = signed out (or account server unreachable).
    pub async fn access_token(&self, app: &AppHandle) -> Option<String> {
        let mut guard = self.access.lock().await;
        if let Some((tok, until)) = guard.as_ref()
            && Instant::now() < *until
        {
            return Some(tok.clone());
        }
        let refresh = crate::cred::load()?;
        let http = app.state::<AppState>().http.clone();
        let res = post(&http, "token.php", &[("grant_type", "refresh_token"), ("client_id", CLIENT_ID), ("refresh_token", &refresh)]).await;
        match res {
            Ok((200, v)) => self.accept_tokens(app, &mut guard, &v).map_err(|e| tracing::error!("token refresh: {e}")).ok(),
            Ok((_, v)) if matches!(v["error"].as_str(), Some("invalid_grant" | "invalid_client")) => {
                tracing::info!("refresh token rejected ({}); signed out", v["error"]);
                *guard = None;
                drop(guard);
                crate::cred::delete();
                self.set_user(app, None);
                None
            }
            Ok((s, v)) => {
                tracing::warn!(status = s, "token refresh failed: {v}");
                None
            }
            Err(e) => {
                tracing::warn!("token refresh failed: {e}");
                None
            }
        }
    }

    /// Forgets the cached access token (server said it's invalid): the next call refreshes.
    pub async fn invalidate(&self) {
        *self.access.lock().await = None;
    }

    /// Starts the device flow: returns the code to show and polls in the background.
    pub async fn start_device(&self, app: &AppHandle) -> Result<DeviceInfo, String> {
        self.cancel_device();
        let http = app.state::<AppState>().http.clone();
        let (status, v) = post(&http, "device.php", &[("client_id", CLIENT_ID)]).await?;
        if status != 200 {
            return Err(format!("account server answered {status}: {}", v["error"].as_str().unwrap_or("?")));
        }
        let info = DeviceInfo {
            user_code: v["user_code"].as_str().ok_or("bad device response")?.to_string(),
            verification_uri: v["verification_uri"].as_str().unwrap_or(ACCOUNTS).to_string(),
            verification_uri_complete: v["verification_uri_complete"].as_str().unwrap_or(ACCOUNTS).to_string(),
            expires_in: v["expires_in"].as_u64().unwrap_or(600),
        };
        let device_code = v["device_code"].as_str().ok_or("bad device response")?.to_string();
        let interval = v["interval"].as_u64().unwrap_or(5).max(1);
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        *self.device.lock().unwrap() = Some(cancel.clone());
        open_verification(app, &info.verification_uri_complete);

        let (app2, expires) = (app.clone(), info.expires_in);
        tauri::async_runtime::spawn(async move {
            let started = Instant::now();
            let mut wait = interval;
            loop {
                tokio::time::sleep(Duration::from_secs(wait)).await;
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if started.elapsed().as_secs() > expires {
                    let _ = app2.emit("auth:device-failed", "The code expired. Please try again.");
                    return;
                }
                let r = post(
                    &http,
                    "token.php",
                    &[
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        ("client_id", CLIENT_ID),
                        ("device_code", &device_code),
                    ],
                )
                .await;
                let auth = &app2.state::<AppState>().auth;
                match r {
                    Ok((200, v)) => {
                        let mut slot = auth.access.lock().await;
                        match auth.accept_tokens(&app2, &mut slot, &v) {
                            Ok(_) => tracing::info!("signed in"),
                            Err(e) => {
                                let _ = app2.emit("auth:device-failed", e);
                            }
                        }
                        return;
                    }
                    Ok((_, v)) => match v["error"].as_str() {
                        Some("authorization_pending") => {}
                        Some("slow_down") => wait += 5,
                        Some("access_denied") => {
                            let _ = app2.emit("auth:device-failed", "Sign-in was cancelled in the browser.");
                            return;
                        }
                        Some("expired_token") => {
                            let _ = app2.emit("auth:device-failed", "The code expired. Please try again.");
                            return;
                        }
                        other => {
                            let _ = app2.emit("auth:device-failed", format!("Sign-in failed ({})", other.unwrap_or("unknown")));
                            return;
                        }
                    },
                    Err(e) => tracing::warn!("device poll: {e}"), // keep trying until expiry
                }
            }
        });
        Ok(info)
    }

    pub fn cancel_device(&self) {
        if let Some(c) = self.device.lock().unwrap().take() {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// Revokes the tokens on the server (best effort) and forgets everything locally.
    pub async fn sign_out(&self, app: &AppHandle) {
        let token = self.access.lock().await.take().map(|(t, _)| t);
        let token = match token {
            Some(t) => Some(t),
            None => self.access_token(app).await,
        };
        if let Some(t) = token {
            let http = app.state::<AppState>().http.clone();
            let _ = http.delete(format!("{ACCOUNTS}api/userinfo.php")).bearer_auth(t).send().await;
        }
        *self.access.lock().await = None;
        crate::cred::delete();
        self.set_user(app, None);
    }
}

/// Opens the account page only for URLs on the account host.
pub fn open_verification(app: &AppHandle, url: &str) {
    if url.starts_with(ACCOUNTS) {
        let _ = app.opener().open_url(url, None::<&str>);
    }
}
