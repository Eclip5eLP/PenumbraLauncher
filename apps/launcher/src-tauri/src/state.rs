//! Shared app state.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use penumbra_core::catalog::{Catalog, Item};

use crate::catalog::CatalogStatus;
use crate::jobs::JobManager;
use crate::library::{self, Library};
use crate::net::Limiter;
use crate::settings::Settings;

pub struct AppState {
    pub started: Instant,
    pub shown: AtomicBool,
    pub settings: Mutex<Settings>,
    pub catalog: RwLock<Catalog>,
    pub catalog_etag: Mutex<Option<String>>,
    pub catalog_status: Mutex<CatalogStatus>,
    pub library: Mutex<Library>,
    /// item id -> when it was started
    pub running: Mutex<HashMap<String, Instant>>,
    pub jobs: JobManager,
    pub http: reqwest::Client,
    pub auth: crate::auth::Auth,
    pub limiter: Limiter,
}

impl AppState {
    pub fn new(started: Instant, settings: Settings) -> Self {
        let (catalog, etag) = crate::catalog::load_cached();
        Self {
            started,
            shown: AtomicBool::new(false),
            settings: Mutex::new(settings),
            catalog: RwLock::new(catalog),
            catalog_etag: Mutex::new(etag),
            catalog_status: Mutex::new(CatalogStatus { state: "loading", message: None }),
            library: Mutex::new(library::load()),
            running: Mutex::new(HashMap::new()),
            jobs: JobManager::default(),
            http: crate::net::client(),
            auth: {
                let a = crate::auth::Auth::default();
                a.load();
                a
            },
            limiter: Limiter::default(),
        }
    }

    pub fn item(&self, id: &str) -> Option<Item> {
        self.catalog.read().unwrap().items.iter().find(|i| i.id == id).cloned()
    }

    pub fn api_base(&self) -> String {
        self.settings.lock().unwrap().api_base.clone()
    }

    /// Updates the library, persists it and returns a copy for the UI event.
    pub fn update_library(&self, f: impl FnOnce(&mut Library)) -> Library {
        let mut lib = self.library.lock().unwrap();
        f(&mut lib);
        library::save(&lib);
        lib.clone()
    }
}
