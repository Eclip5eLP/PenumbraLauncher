//! Shared app state. M1: catalog and library are mock data; real sources replace them in M3.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Instant;

use penumbra_core::catalog::Catalog;
use serde::Serialize;

use crate::jobs::JobManager;
use crate::settings::Settings;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub version: String,
    pub channel: String,
    pub path: String,
    /// Bytes on disk.
    pub size: u64,
    pub playtime_sec: u64,
    /// Unix seconds.
    pub last_played: Option<u64>,
}

pub type Library = BTreeMap<String, Installed>;

pub struct AppState {
    pub started: Instant,
    pub shown: AtomicBool,
    pub settings: Mutex<Settings>,
    pub catalog: Catalog,
    pub library: Mutex<Library>,
    pub running: Mutex<HashSet<String>>,
    pub jobs: JobManager,
}

impl AppState {
    pub fn new(started: Instant, settings: Settings) -> Self {
        let catalog: Catalog =
            serde_json::from_str(include_str!("../mock/catalog.json")).expect("mock catalog is valid JSON");
        let mut library = Library::new();
        library.insert(
            "pgbot".into(),
            Installed {
                version: "v0.9.2".into(),
                channel: "stable".into(),
                path: r"D:\Penumbra Games\Pirate Galaxy AFK Bot".into(),
                size: 40_100_000,
                playtime_sec: 3 * 3600 + 25 * 60,
                last_played: Some(1_763_500_000),
            },
        );
        Self {
            started,
            shown: AtomicBool::new(false),
            settings: Mutex::new(settings),
            catalog,
            library: Mutex::new(library),
            running: Mutex::new(HashSet::new()),
            jobs: JobManager::default(),
        }
    }

    pub fn item(&self, id: &str) -> Option<&penumbra_core::catalog::Item> {
        self.catalog.items.iter().find(|i| i.id == id)
    }
}
