//! Launcher settings (docs/07-settings.md). Stored as `%APPDATA%\Penumbra Launcher\settings.json`.
//! Unknown/missing fields fall back to defaults so old files keep loading.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::paths;

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema: u32,
    // General
    pub language: String,
    pub start_with_windows: bool,
    pub start_minimized: bool,
    /// `tray` | `exit`
    pub close_button: String,
    /// `minimize` | `tray` | `stay` | `close`
    pub on_game_launch: String,
    /// `stable` | `beta`
    pub launcher_channel: String,
    pub auto_update_launcher: bool,
    pub notifications: bool,
    // Downloads
    pub libraries: Vec<String>,
    pub default_library: Option<String>,
    pub auto_update_games: bool,
    pub max_concurrent_jobs: u32,
    pub connections_per_job: u32,
    /// KB/s, 0 = unlimited
    pub bandwidth_limit: u32,
    pub pause_while_running: bool,
    pub resume_on_start: bool,
    pub verify_after_install: bool,
    /// `ask` | `always` | `never`
    pub create_desktop_shortcut: String,
    // Appearance
    pub background_video: bool,
    /// `system` | `on` | `off`
    pub reduce_motion: String,
    /// `none` | `mica`
    pub window_effect: String,
    pub ui_scale: f32,
    pub use_game_accent: bool,
    pub rail_order: Vec<String>,
    pub hidden_items: Vec<String>,
    pub pinned_items: Vec<String>,
    // Account & cloud
    pub sync_settings: bool,
    pub cloud_saves: bool,
    pub share_token_with_games: bool,
    // Advanced
    pub log_level: String,
    pub api_base: String,
    pub offline_mode: bool,
    pub cache_limit_mb: u32,
    pub dev_mode: bool,
    /// Unix time of the last change to a synced setting (newest wins in settings sync).
    pub updated_at: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            language: "en".into(),
            start_with_windows: false,
            start_minimized: false,
            close_button: "tray".into(),
            on_game_launch: "minimize".into(),
            launcher_channel: "stable".into(),
            auto_update_launcher: true,
            notifications: true,
            libraries: vec![],
            default_library: None,
            auto_update_games: true,
            max_concurrent_jobs: 1,
            connections_per_job: 8,
            bandwidth_limit: 0,
            pause_while_running: true,
            resume_on_start: true,
            verify_after_install: false,
            create_desktop_shortcut: "ask".into(),
            background_video: true,
            reduce_motion: "system".into(),
            window_effect: "none".into(),
            ui_scale: 1.0,
            use_game_accent: true,
            rail_order: vec![],
            hidden_items: vec![],
            pinned_items: vec![],
            sync_settings: true,
            cloud_saves: true,
            share_token_with_games: false,
            log_level: "info".into(),
            api_base: "https://eclip5e.com/penumbra/".into(),
            offline_mode: false,
            cache_limit_mb: 1024,
            dev_mode: cfg!(debug_assertions),
            updated_at: 0,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        paths::config_dir().join("settings.json")
    }

    /// Loads settings; a missing or unreadable file gives defaults (a broken file is kept as `.bak`).
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => s.sanitized(),
                Err(e) => {
                    tracing::warn!("settings.json unreadable ({e}), using defaults");
                    let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> AppResult<()> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| AppError::Settings(e.to_string()))?;
        paths::write_atomic(&Self::path(), &json)?;
        Ok(())
    }

    /// Applies a partial JSON object (`{"bandwidthLimit": 500}`) and returns the new settings.
    pub fn patched(&self, patch: &Value) -> AppResult<Settings> {
        let Value::Object(patch) = patch else {
            return Err(AppError::Settings("patch must be an object".into()));
        };
        let mut cur = serde_json::to_value(self).map_err(|e| AppError::Settings(e.to_string()))?;
        let obj = cur.as_object_mut().expect("settings serialize to an object");
        for (k, v) in patch {
            if !obj.contains_key(k) {
                return Err(AppError::Settings(format!("unknown setting `{k}`")));
            }
            obj.insert(k.clone(), v.clone());
        }
        let next: Settings = serde_json::from_value(cur).map_err(|e| AppError::Settings(e.to_string()))?;
        Ok(next.sanitized())
    }

    /// Clamp numbers and normalize enums so a hand-edited file can't break the app.
    fn sanitized(mut self) -> Self {
        let d = Settings::default();
        let pick = |v: &mut String, allowed: &[&str], def: &str| {
            if !allowed.contains(&v.as_str()) {
                *v = def.into();
            }
        };
        pick(&mut self.close_button, &["tray", "exit"], &d.close_button);
        pick(&mut self.on_game_launch, &["minimize", "tray", "stay", "close"], &d.on_game_launch);
        pick(&mut self.launcher_channel, &["stable", "beta"], &d.launcher_channel);
        pick(&mut self.create_desktop_shortcut, &["ask", "always", "never"], &d.create_desktop_shortcut);
        pick(&mut self.reduce_motion, &["system", "on", "off"], &d.reduce_motion);
        pick(&mut self.window_effect, &["none", "mica"], &d.window_effect);
        pick(&mut self.log_level, &["error", "warn", "info", "debug", "trace"], &d.log_level);
        self.max_concurrent_jobs = self.max_concurrent_jobs.clamp(1, 3);
        self.connections_per_job = self.connections_per_job.clamp(1, 32);
        self.ui_scale = if self.ui_scale.is_finite() { self.ui_scale.clamp(0.8, 1.25) } else { 1.0 };
        self.cache_limit_mb = self.cache_limit_mb.clamp(64, 65536);
        self.schema = SCHEMA;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_applies_and_clamps() {
        let s = Settings::default();
        let n = s.patched(&json!({ "connectionsPerJob": 99, "closeButton": "exit" })).unwrap();
        assert_eq!(n.connections_per_job, 32);
        assert_eq!(n.close_button, "exit");
    }

    #[test]
    fn patch_rejects_unknown_keys_and_bad_types() {
        let s = Settings::default();
        assert!(s.patched(&json!({ "nope": 1 })).is_err());
        assert!(s.patched(&json!({ "bandwidthLimit": "fast" })).is_err());
    }

    #[test]
    fn old_files_with_missing_fields_load() {
        let s: Settings = serde_json::from_str(r#"{ "language": "de" }"#).unwrap();
        assert_eq!(s.language, "de");
        assert_eq!(s.connections_per_job, 8);
    }
}
