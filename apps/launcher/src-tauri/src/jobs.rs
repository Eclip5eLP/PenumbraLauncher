//! Download queue (docs/05-launcher-app.md#job-state-machine). Runs engine jobs, turns their counters into
//! throttled `jobs:changed` events (<= 10 Hz) and remembers unfinished jobs across restarts (`jobs.json`).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::engine::{self, Kind, Progress};
use crate::error::{AppError, AppResult};
use crate::library::{Installed, now_unix};
use crate::paths;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobKind {
    Install,
    Update,
    Repair,
}

impl JobKind {
    fn engine(self) -> Kind {
        match self {
            JobKind::Install => Kind::Install,
            JobKind::Update => Kind::Update,
            JobKind::Repair => Kind::Repair,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Preparing,
    Verifying,
    Downloading,
    Paused,
    Finalizing,
    Done,
    Error,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(self, JobState::Done | JobState::Error | JobState::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: u64,
    pub item: String,
    pub kind: JobKind,
    pub channel: String,
    pub version: String,
    pub path: String,
    pub state: JobState,
    pub downloaded: u64,
    pub total: u64,
    /// Bytes per second (smoothed).
    pub speed: u64,
    /// Seconds remaining.
    pub eta: Option<u64>,
    pub error: Option<String>,
    pub note: Option<String>,
    pub finished_at: Option<u64>,
}

/// What survives a restart.
#[derive(Serialize, Deserialize)]
struct Saved {
    item: String,
    kind: JobKind,
    channel: String,
    version: String,
    path: String,
    paused: bool,
}

struct Entry {
    view: JobView,
    progress: Arc<Progress>,
    handle: Option<tauri::async_runtime::JoinHandle<()>>,
    last_done: u64,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    entries: Vec<Entry>,
    dirty: bool,
}

#[derive(Default, Clone)]
pub struct JobManager {
    inner: Arc<Mutex<Inner>>,
}

fn saved_file() -> PathBuf {
    paths::config_dir().join("jobs.json")
}

impl JobManager {
    pub fn list(&self) -> Vec<JobView> {
        self.inner.lock().unwrap().entries.iter().map(|e| e.view.clone()).collect()
    }

    pub fn has_active_job(&self, item: &str) -> bool {
        self.inner.lock().unwrap().entries.iter().any(|e| e.view.item == item && !e.view.state.is_finished())
    }

    pub fn enqueue(&self, item: &str, kind: JobKind, channel: &str, version: &str, path: &str) -> AppResult<JobView> {
        let mut g = self.inner.lock().unwrap();
        if g.entries.iter().any(|e| e.view.item == item && !e.view.state.is_finished()) {
            return Err(AppError::InvalidState(format!("{item} already has a download")));
        }
        g.next_id += 1;
        let view = JobView {
            id: g.next_id,
            item: item.into(),
            kind,
            channel: channel.into(),
            version: version.into(),
            path: path.into(),
            state: JobState::Queued,
            downloaded: 0,
            total: 0,
            speed: 0,
            eta: None,
            error: None,
            note: None,
            finished_at: None,
        };
        g.entries.push(Entry { view: view.clone(), progress: Arc::default(), handle: None, last_done: 0 });
        g.dirty = true;
        drop(g);
        self.persist();
        Ok(view)
    }

    fn with_entry<T>(&self, id: u64, f: impl FnOnce(&mut Entry) -> AppResult<T>) -> AppResult<T> {
        let mut g = self.inner.lock().unwrap();
        let e = g.entries.iter_mut().find(|e| e.view.id == id).ok_or(AppError::UnknownJob(id))?;
        let r = f(e)?;
        g.dirty = true;
        drop(g);
        self.persist();
        Ok(r)
    }

    pub fn pause(&self, id: u64) -> AppResult<()> {
        self.with_entry(id, |e| {
            if e.view.state.is_finished() || e.view.state == JobState::Paused {
                return Err(AppError::InvalidState("can't pause this job".into()));
            }
            if e.view.state == JobState::Finalizing {
                return Err(AppError::InvalidState("almost done, can't pause now".into()));
            }
            e.progress.stop.store(true, Ordering::Relaxed);
            if let Some(h) = e.handle.take() {
                h.abort(); // staging + journal stay: resume continues
            }
            e.view.state = JobState::Paused;
            e.view.speed = 0;
            e.view.eta = None;
            Ok(())
        })
    }

    pub fn resume(&self, id: u64) -> AppResult<()> {
        self.with_entry(id, |e| {
            if e.view.state != JobState::Paused {
                return Err(AppError::InvalidState("job isn't paused".into()));
            }
            e.view.state = JobState::Queued;
            e.view.error = None;
            Ok(())
        })
    }

    pub fn cancel(&self, id: u64) -> AppResult<()> {
        let (path, fresh) = self.with_entry(id, |e| {
            if e.view.state.is_finished() || e.view.state == JobState::Finalizing {
                return Err(AppError::InvalidState("job can't be cancelled now".into()));
            }
            e.progress.stop.store(true, Ordering::Relaxed);
            if let Some(h) = e.handle.take() {
                h.abort();
            }
            e.view.state = JobState::Cancelled;
            e.view.speed = 0;
            e.view.eta = None;
            e.view.finished_at = Some(now_unix());
            Ok((PathBuf::from(&e.view.path), e.view.kind == JobKind::Install))
        })?;
        // Clean up off the UI thread (discard waits for in-flight writes to let go of their files).
        std::thread::spawn(move || engine::discard(&path, fresh));
        Ok(())
    }

    /// Retries a failed job (keeps what was already downloaded).
    pub fn retry(&self, id: u64) -> AppResult<()> {
        self.with_entry(id, |e| {
            if e.view.state != JobState::Error {
                return Err(AppError::InvalidState("job hasn't failed".into()));
            }
            e.view.state = JobState::Queued;
            e.view.error = None;
            e.view.finished_at = None;
            Ok(())
        })
    }

    pub fn prioritize(&self, id: u64) -> AppResult<()> {
        let mut g = self.inner.lock().unwrap();
        let pos = g.entries.iter().position(|e| e.view.id == id).ok_or(AppError::UnknownJob(id))?;
        let e = g.entries.remove(pos);
        g.entries.insert(0, e);
        g.dirty = true;
        Ok(())
    }

    pub fn clear_finished(&self) {
        let mut g = self.inner.lock().unwrap();
        g.entries.retain(|e| !e.view.state.is_finished());
        g.dirty = true;
    }

    fn persist(&self) {
        let saved: Vec<Saved> = self
            .inner
            .lock()
            .unwrap()
            .entries
            .iter()
            .filter(|e| !e.view.state.is_finished())
            .map(|e| Saved {
                item: e.view.item.clone(),
                kind: e.view.kind,
                channel: e.view.channel.clone(),
                version: e.view.version.clone(),
                path: e.view.path.clone(),
                paused: e.view.state == JobState::Paused,
            })
            .collect();
        if let Ok(json) = serde_json::to_vec_pretty(&saved) {
            let _ = paths::write_atomic(&saved_file(), &json);
        }
    }

    /// Restores unfinished jobs from the last session.
    pub fn restore(&self, resume: bool) {
        let Ok(bytes) = std::fs::read(saved_file()) else { return };
        let Ok(saved) = serde_json::from_slice::<Vec<Saved>>(&bytes) else { return };
        for s in saved {
            if let Ok(v) = self.enqueue(&s.item, s.kind, &s.channel, &s.version, &s.path)
                && (s.paused || !resume)
            {
                let _ = self.with_entry(v.id, |e| {
                    e.view.state = JobState::Paused;
                    Ok(())
                });
            }
        }
    }

    pub fn start(&self, app: AppHandle) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_millis(100));
            iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                iv.tick().await;
                if this.tick(&app) {
                    let _ = app.emit("jobs:changed", this.list());
                }
            }
        });
    }

    /// Starts queued jobs when there's capacity and refreshes progress. Returns true when the UI must update.
    fn tick(&self, app: &AppHandle) -> bool {
        let max = app.state::<AppState>().settings.lock().unwrap().max_concurrent_jobs.max(1) as usize;
        let mut g = self.inner.lock().unwrap();
        let mut changed = std::mem::take(&mut g.dirty);
        let mut active = g.entries.iter().filter(|e| e.handle.is_some()).count();
        for e in g.entries.iter_mut() {
            if active < max && e.view.state == JobState::Queued {
                active += 1;
                changed = true;
                e.view.state = JobState::Preparing;
                e.progress = Arc::default();
                e.last_done = 0;
                let spec = engine::Spec {
                    item: e.view.item.clone(),
                    kind: e.view.kind.engine(),
                    channel: e.view.channel.clone(),
                    version: e.view.version.clone(),
                    path: PathBuf::from(&e.view.path),
                };
                let (app2, prog, id, this) = (app.clone(), e.progress.clone(), e.view.id, self.clone());
                e.handle = Some(tauri::async_runtime::spawn(async move {
                    let r = engine::run(app2.clone(), spec, prog).await;
                    this.finish(&app2, id, r);
                }));
            }
            if e.handle.is_some() {
                let p = &e.progress;
                let state = match p.phase.load(Ordering::Relaxed) {
                    engine::PHASE_VERIFYING => JobState::Verifying,
                    engine::PHASE_DOWNLOADING => JobState::Downloading,
                    engine::PHASE_FINALIZING => JobState::Finalizing,
                    _ => JobState::Preparing,
                };
                let done = p.done.load(Ordering::Relaxed);
                let total = p.total.load(Ordering::Relaxed);
                let inst = done.saturating_sub(e.last_done) * 10;
                e.last_done = done;
                let speed = if state == JobState::Downloading || state == JobState::Verifying {
                    if e.view.speed == 0 { inst } else { (e.view.speed * 8 + inst * 2) / 10 }
                } else {
                    0
                };
                let eta = (speed > 0).then(|| total.saturating_sub(done) / speed.max(1));
                if state != e.view.state || done != e.view.downloaded || total != e.view.total || speed != e.view.speed {
                    changed = true;
                }
                e.view.state = state;
                e.view.downloaded = done;
                e.view.total = total;
                e.view.speed = speed;
                e.view.eta = eta;
            }
        }
        changed
    }

    fn finish(&self, app: &AppHandle, id: u64, result: Result<engine::Outcome, String>) {
        let state = app.state::<AppState>();
        let mut installed = None;
        {
            let mut g = self.inner.lock().unwrap();
            let Some(e) = g.entries.iter_mut().find(|e| e.view.id == id) else { return };
            e.handle = None;
            e.view.speed = 0;
            e.view.eta = None;
            e.view.finished_at = Some(now_unix());
            match result {
                Ok(o) => {
                    e.view.state = JobState::Done;
                    e.view.note = o.note;
                    e.view.downloaded = e.view.total;
                    installed = Some((e.view.item.clone(), e.view.version.clone(), e.view.channel.clone(), e.view.path.clone(), o.size));
                    tracing::info!(item = %e.view.item, version = %e.view.version, written = o.written, "job done");
                }
                Err(err) => {
                    tracing::error!(item = %e.view.item, "job failed: {err}");
                    e.view.state = JobState::Error;
                    e.view.error = Some(err);
                }
            }
            g.dirty = true;
        }
        self.persist();
        if let Some((item, version, channel, path, size)) = installed {
            let lib = state.update_library(|lib| {
                let prev = lib.get(&item).cloned();
                lib.insert(
                    item.clone(),
                    Installed {
                        version,
                        channel,
                        path: path.clone(),
                        size,
                        playtime_sec: prev.as_ref().map(|p| p.playtime_sec).unwrap_or(0),
                        last_played: prev.and_then(|p| p.last_played),
                    },
                );
            });
            // Remember the library folder for the next install.
            if let Some(parent) = PathBuf::from(&path).parent().map(|p| p.to_string_lossy().to_string()) {
                let mut s = state.settings.lock().unwrap();
                if !s.libraries.contains(&parent) {
                    s.libraries.push(parent.clone());
                }
                if s.default_library.is_none() {
                    s.default_library = Some(parent);
                }
                let _ = s.save();
            }
            let _ = app.emit("library:changed", lib);
        }
    }
}
