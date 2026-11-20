//! Download job queue. M1: a **simulated** downloader that exercises the real UI contract (queue, pause, resume,
//! cancel, throttled progress events). The real engine (docs/05-launcher-app.md#download-engine-details)
//! replaces `tick()` in M3; the public surface stays.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::state::{AppState, Installed};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Repair is created by verify & repair (M3)
pub enum JobKind {
    Install,
    Update,
    Repair,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Error is produced by the real downloader (M3)
pub enum JobState {
    Queued,
    Preparing,
    Downloading,
    Paused,
    Finalizing,
    Done,
    Error,
    Cancelled,
}

impl JobState {
    pub fn is_active(self) -> bool {
        matches!(self, JobState::Preparing | JobState::Downloading | JobState::Finalizing)
    }
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
    /// Unix seconds when finished.
    pub finished_at: Option<u64>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    jobs: Vec<JobView>,
    /// Ticks spent in the current preparing/finalizing phase.
    phase_ticks: std::collections::HashMap<u64, u32>,
    t: u64,
}

#[derive(Default, Clone)]
pub struct JobManager {
    inner: Arc<Mutex<Inner>>,
}

const TICK: Duration = Duration::from_millis(100);

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl JobManager {
    pub fn list(&self) -> Vec<JobView> {
        self.inner.lock().unwrap().jobs.clone()
    }

    pub fn enqueue(&self, item: &str, kind: JobKind, channel: &str, version: &str, path: &str, total: u64) -> AppResult<JobView> {
        let mut g = self.inner.lock().unwrap();
        if g.jobs.iter().any(|j| j.item == item && !j.state.is_finished()) {
            return Err(AppError::InvalidState(format!("{item} already has a download")));
        }
        g.next_id += 1;
        let job = JobView {
            id: g.next_id,
            item: item.into(),
            kind,
            channel: channel.into(),
            version: version.into(),
            path: path.into(),
            state: JobState::Queued,
            downloaded: 0,
            total,
            speed: 0,
            eta: None,
            error: None,
            finished_at: None,
        };
        g.jobs.push(job.clone());
        Ok(job)
    }

    pub fn set_paused(&self, id: u64, paused: bool) -> AppResult<()> {
        let mut g = self.inner.lock().unwrap();
        let j = g.jobs.iter_mut().find(|j| j.id == id).ok_or(AppError::UnknownJob(id))?;
        match (paused, j.state) {
            (true, JobState::Queued | JobState::Preparing | JobState::Downloading) => {
                j.state = JobState::Paused;
                j.speed = 0;
                j.eta = None;
            }
            (false, JobState::Paused) => j.state = JobState::Queued,
            _ => return Err(AppError::InvalidState(format!("can't {} job in state {:?}", if paused { "pause" } else { "resume" }, j.state))),
        }
        Ok(())
    }

    pub fn cancel(&self, id: u64) -> AppResult<()> {
        let mut g = self.inner.lock().unwrap();
        let j = g.jobs.iter_mut().find(|j| j.id == id).ok_or(AppError::UnknownJob(id))?;
        if j.state.is_finished() || j.state == JobState::Finalizing {
            return Err(AppError::InvalidState("job can't be cancelled now".into()));
        }
        j.state = JobState::Cancelled;
        j.speed = 0;
        j.eta = None;
        j.finished_at = Some(now_unix());
        Ok(())
    }

    /// Moves a queued/paused job to the front of the queue.
    pub fn prioritize(&self, id: u64) -> AppResult<()> {
        let mut g = self.inner.lock().unwrap();
        let pos = g.jobs.iter().position(|j| j.id == id).ok_or(AppError::UnknownJob(id))?;
        let job = g.jobs.remove(pos);
        g.jobs.insert(0, job);
        Ok(())
    }

    pub fn clear_finished(&self) {
        self.inner.lock().unwrap().jobs.retain(|j| !j.state.is_finished());
    }

    /// Starts the background loop. Emits `jobs:changed` (full list) at most every 100 ms, only on change.
    pub fn start(&self, app: AppHandle) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut iv = tokio::time::interval(TICK);
            iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                iv.tick().await;
                let (changed, finished) = this.tick(&app);
                if changed {
                    let _ = app.emit("jobs:changed", this.list());
                }
                for job in finished {
                    on_finished(&app, &job);
                }
            }
        });
    }

    /// One simulation step. Returns (anything changed, jobs that just completed).
    fn tick(&self, app: &AppHandle) -> (bool, Vec<JobView>) {
        let state = app.state::<AppState>();
        let (max_jobs, limit_kbs, pause_running) = {
            let s = state.settings.lock().unwrap();
            (s.max_concurrent_jobs as usize, s.bandwidth_limit as u64, s.pause_while_running)
        };
        let game_running = pause_running && !state.running.lock().unwrap().is_empty();

        let mut g = self.inner.lock().unwrap();
        g.t += 1;
        let t = g.t;
        let mut changed = false;
        let mut finished = vec![];

        // Promote queued jobs while there's capacity (queue order = list order).
        let mut active = g.jobs.iter().filter(|j| j.state.is_active()).count();
        for j in g.jobs.iter_mut() {
            if active >= max_jobs {
                break;
            }
            if j.state == JobState::Queued {
                j.state = JobState::Preparing;
                active += 1;
                changed = true;
            }
        }

        let Inner { jobs, phase_ticks, .. } = &mut *g;
        for j in jobs.iter_mut() {
            match j.state {
                JobState::Preparing => {
                    let n = phase_ticks.entry(j.id).or_default();
                    *n += 1;
                    if *n >= 6 {
                        phase_ticks.remove(&j.id);
                        j.state = JobState::Downloading;
                    }
                    changed = true;
                }
                JobState::Downloading => {
                    // Simulated link: ~55 MB/s with a gentle wobble, capped by the bandwidth limit.
                    let wobble = 0.85 + 0.15 * ((t as f64) / 7.0).sin();
                    let mut bps = (55_000_000.0 * wobble) as u64;
                    if limit_kbs > 0 {
                        bps = bps.min(limit_kbs * 1024);
                    }
                    if game_running {
                        bps = 0;
                    }
                    let step = bps / 10;
                    j.downloaded = (j.downloaded + step).min(j.total);
                    j.speed = if j.speed == 0 { bps } else { (j.speed * 7 + bps * 3) / 10 };
                    j.eta = (j.speed > 0).then(|| (j.total - j.downloaded) / j.speed.max(1));
                    if j.downloaded >= j.total {
                        j.state = JobState::Finalizing;
                        j.speed = 0;
                        j.eta = None;
                    }
                    changed = true;
                }
                JobState::Finalizing => {
                    let n = phase_ticks.entry(j.id).or_default();
                    *n += 1;
                    if *n >= 8 {
                        phase_ticks.remove(&j.id);
                        j.state = JobState::Done;
                        j.finished_at = Some(now_unix());
                        finished.push(j.clone());
                    }
                    changed = true;
                }
                _ => {}
            }
        }
        (changed, finished)
    }
}

fn on_finished(app: &AppHandle, job: &JobView) {
    let state = app.state::<AppState>();
    let size = state
        .item(&job.item)
        .and_then(|i| i.channels.get(&job.channel))
        .and_then(|c| c.size)
        .unwrap_or(job.total);
    {
        let mut lib = state.library.lock().unwrap();
        let prev = lib.get(&job.item).cloned();
        lib.insert(
            job.item.clone(),
            Installed {
                version: job.version.clone(),
                channel: job.channel.clone(),
                path: job.path.clone(),
                size,
                playtime_sec: prev.as_ref().map(|p| p.playtime_sec).unwrap_or(0),
                last_played: prev.and_then(|p| p.last_played),
            },
        );
    }
    tracing::info!(item = %job.item, version = %job.version, "job finished (simulated)");
    let _ = app.emit("library:changed", state.library.lock().unwrap().clone());
}
