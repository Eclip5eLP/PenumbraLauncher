//! Catalog: disk cache for instant startup + background refresh from the server (docs/05-launcher-app.md).
//! Every item's signature is checked against the embedded public key; unsigned or tampered items are dropped.

use penumbra_core::catalog::{Catalog, Item};
use penumbra_core::sign::{self, VerifyingKey};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;
use crate::state::AppState;

/// Public key of the publisher (keys/manifest_ed25519.pub). Also verifies manifests.
const PUBLIC_KEY: &str = include_str!("../../../../keys/manifest_ed25519.pub");

pub fn public_key() -> VerifyingKey {
    sign::verifying_key_from_b64(PUBLIC_KEY).expect("embedded public key is valid")
}

fn cache_file() -> std::path::PathBuf {
    paths::local_dir().join("cache").join("catalog.json")
}

/// Parses a catalog body, keeping only items with a valid signature.
pub fn parse(body: &[u8], key: &VerifyingKey) -> Result<Catalog, String> {
    parse_full(body, key).map(|(c, _)| c)
}

/// Like [`parse`], also returning the unsigned GitHub resolution map (None when the server sent none).
pub fn parse_full(body: &[u8], key: &VerifyingKey) -> Result<(Catalog, Option<crate::github::ResolvedMap>), String> {
    let mut v: Value = serde_json::from_slice(body).map_err(|e| format!("catalog is not valid JSON: {e}"))?;
    let raw_items = v.get_mut("items").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
    let resolved = v
        .as_object_mut()
        .and_then(|o| o.remove("resolved"))
        .and_then(|r| serde_json::from_value::<crate::github::ResolvedMap>(r).ok());
    let mut catalog: Catalog = serde_json::from_value(v).map_err(|e| format!("catalog format: {e}"))?;
    for raw in raw_items {
        let id = raw.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
        if let Err(e) = sign::verify_item(key, &raw) {
            tracing::warn!(%id, "dropping catalog item: {e}");
            continue;
        }
        match serde_json::from_value::<Item>(raw) {
            Ok(item) => catalog.items.push(item),
            Err(e) => tracing::warn!(%id, "dropping catalog item with unknown format: {e}"),
        }
    }
    if let Some(r) = &resolved {
        crate::github::apply(&mut catalog, r);
    }
    Ok((catalog, resolved))
}

/// Cached catalog from the last successful refresh (empty on first start).
pub fn load_cached() -> (Catalog, Option<String>) {
    let Ok(bytes) = std::fs::read(cache_file()) else { return (Catalog::default(), None) };
    let etag = std::fs::read_to_string(cache_file().with_extension("etag")).ok();
    match parse(&bytes, &public_key()) {
        Ok(c) => (c, etag),
        Err(e) => {
            tracing::warn!("cached catalog unusable: {e}");
            (Catalog::default(), None)
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    /// `loading` | `ok` | `offline` | `error`
    pub state: &'static str,
    pub message: Option<String>,
}

fn status(app: &AppHandle, state: &'static str, message: Option<String>) {
    let s = CatalogStatus { state, message };
    *app.state::<AppState>().catalog_status.lock().unwrap() = s.clone();
    let _ = app.emit("catalog:status", s);
}

/// Fetches the catalog (conditional GET), verifies, caches, swaps it into the state and notifies the UI.
pub async fn refresh(app: AppHandle) {
    let state = app.state::<AppState>();
    let (base, offline) = {
        let s = state.settings.lock().unwrap();
        (s.api_base.clone(), s.offline_mode)
    };
    if offline {
        status(&app, "offline", None);
        return;
    }
    status(&app, "loading", None);
    let etag = state.catalog_etag.lock().unwrap().clone();
    let mut resp = None;
    // Signed in: personalised catalog. A rejected token is refreshed once and the request repeated.
    for attempt in 0..2 {
        let mut req = state.http.get(format!("{base}api/v1/catalog.php"));
        if let Some(e) = &etag {
            req = req.header("If-None-Match", e);
        }
        if let Some(tok) = state.auth.access_token(&app).await {
            req = req.bearer_auth(tok);
        }
        let r = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("catalog refresh failed: {e}");
                status(&app, "error", Some("Can't reach the Penumbra server. Showing the last known catalog.".into()));
                return;
            }
        };
        let rejected = r.headers().get("x-penumbra-auth").is_some_and(|v| v == "invalid");
        if rejected && attempt == 0 {
            state.auth.invalidate().await;
            continue;
        }
        resp = Some(r);
        break;
    }
    let Some(resp) = resp else { return };
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        status(&app, "ok", None);
        crate::jobs::auto_update(&app);
        return;
    }
    if !resp.status().is_success() {
        status(&app, "error", Some(format!("Server answered {}", resp.status())));
        return;
    }
    let new_etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).map(String::from);
    let body = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            status(&app, "error", Some(e.to_string()));
            return;
        }
    };
    let catalog = match parse_full(&body, &public_key()) {
        Ok((c, Some(_))) => c,
        Ok((mut c, None)) => {
            // Server didn't resolve GitHub channels: ask GitHub directly.
            crate::github::resolve_direct(&state.http, &mut c).await;
            c
        }
        Err(e) => {
            status(&app, "error", Some(e));
            return;
        }
    };
    let file = cache_file();
    if let Err(e) = paths::write_atomic(&file, &body) {
        tracing::warn!("can't cache catalog: {e}");
    } else if let Some(e) = &new_etag {
        let _ = std::fs::write(file.with_extension("etag"), e);
    }
    tracing::info!(items = catalog.items.len(), "catalog refreshed");
    *state.catalog_etag.lock().unwrap() = new_etag;
    *state.catalog.write().unwrap() = catalog.clone();
    let _ = app.emit("catalog:changed", catalog);
    status(&app, "ok", None);
    crate::jobs::auto_update(&app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use penumbra_core::sign::{SigningKey, sign_item};
    use serde_json::json;

    #[test]
    fn keeps_signed_items_and_drops_tampered_ones() {
        let k = SigningKey::from_bytes(&[3u8; 32]);
        let mut good = json!({ "id": "a", "type": "game", "group": "games", "name": "A" });
        sign_item(&k, &mut good);
        let mut bad = json!({ "id": "b", "type": "game", "group": "games", "name": "B" });
        sign_item(&k, &mut bad);
        bad["name"] = json!("Evil");
        let body = json!({ "format": 1, "generated": "x", "items": [good, bad, { "id": "c" }] });
        let c = parse(body.to_string().as_bytes(), &k.verifying_key()).unwrap();
        assert_eq!(c.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }

    #[test]
    fn embedded_key_parses() {
        let _ = public_key();
    }
}
