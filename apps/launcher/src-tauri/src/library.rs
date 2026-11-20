//! Installed items: `%APPDATA%\Penumbra Launcher\library.json` plus, inside every install folder,
//! `.penumbra/install.json` + `.penumbra/manifest.json` so installs can be found again after a reinstall.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use penumbra_core::manifest::Manifest;
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub version: String,
    pub channel: String,
    pub path: String,
    /// Bytes on disk (from the manifest).
    pub size: u64,
    #[serde(default)]
    pub playtime_sec: u64,
    /// Unix seconds.
    #[serde(default)]
    pub last_played: Option<u64>,
}

pub type Library = BTreeMap<String, Installed>;

fn file() -> PathBuf {
    paths::config_dir().join("library.json")
}

pub fn load() -> Library {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(lib: &Library) {
    match serde_json::to_vec_pretty(lib) {
        Ok(json) => {
            if let Err(e) = paths::write_atomic(&file(), &json) {
                tracing::error!("saving library.json failed: {e}");
            }
        }
        Err(e) => tracing::error!("serializing library failed: {e}"),
    }
}

/// `<install>/.penumbra`
pub fn meta_dir(install: &Path) -> PathBuf {
    install.join(".penumbra")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallRecord {
    pub item: String,
    pub version: String,
    pub channel: String,
}

pub fn read_manifest(install: &Path) -> Option<Manifest> {
    let bytes = std::fs::read(meta_dir(install).join("manifest.json")).ok()?;
    let m: Manifest = serde_json::from_slice(&bytes).ok()?;
    m.validate().ok()?;
    Some(m)
}

pub fn read_record(install: &Path) -> Option<InstallRecord> {
    serde_json::from_slice(&std::fs::read(meta_dir(install).join("install.json")).ok()?).ok()
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
