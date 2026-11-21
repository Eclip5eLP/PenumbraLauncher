//! Penumbra Launcher backend. Architecture: docs/02-architecture.md, docs/05-launcher-app.md.

mod archive;
mod auth;
mod catalog;
mod cloudsave;
mod commands;
mod desktop;
mod cred;
mod engine;
mod error;
mod github;
mod jobs;
mod library;
mod net;
mod paths;
mod process;
mod settings;
mod state;
mod sync;
mod sys;
mod updater;

use std::time::{Duration, Instant};

use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

use crate::settings::Settings;
use crate::state::AppState;

fn init_logging(level: &str) {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::new(format!("warn,penumbra_launcher_lib={level},penumbra_core={level}"));
    // The appender prunes old files at startup and complains if the folder doesn't exist yet.
    let _ = std::fs::create_dir_all(paths::logs_dir());
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("launcher")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths::logs_dir());
    let registry = tracing_subscriber::registry().with(filter);
    match file {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            // The app exits via process::exit, so the guard would never drop anyway; keep the writer alive.
            std::mem::forget(guard);
            let file_layer = fmt::layer().with_ansi(false).with_writer(writer);
            if cfg!(debug_assertions) {
                registry.with(file_layer).with(fmt::layer()).init();
            } else {
                registry.with(file_layer).init();
            }
        }
        Err(_) => registry.with(fmt::layer()).init(),
    }
}

pub fn run() {
    let started = Instant::now();
    let settings = Settings::load();
    init_logging(&settings.log_level);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");

    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the running window instead.
        // A `penumbra://` link opened while we run arrives here as an argument of the second start.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            desktop::show(app);
            for a in args.iter().filter(|a| a.starts_with("penumbra://")) {
                desktop::handle_url(app, a);
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                // We show the window ourselves after the UI's first paint.
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        // Close button: hide to tray (setting) instead of quitting.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window.app_handle().state::<AppState>().settings.lock().unwrap().close_button == "tray";
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .manage(AppState::new(started, settings))
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            if let Err(e) = desktop::create_tray(&handle) {
                tracing::warn!("tray icon: {e}");
            }
            // Started by Windows at sign-in with "start minimized": stay in the tray.
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == "--autostart") && state.settings.lock().unwrap().start_minimized {
                state.start_hidden.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            // Started by a `penumbra://` link: the UI picks it up once it's ready.
            for a in args.iter().filter(|a| a.starts_with("penumbra://")) {
                desktop::handle_url(&handle, a);
            }
            // Release builds register the link scheme for the current exe (the installer does it as well).
            if !cfg!(debug_assertions) {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register_all() {
                    tracing::warn!("registering penumbra:// failed: {e}");
                }
            }
            let resume = state.settings.lock().unwrap().resume_on_start;
            state.jobs.restore(resume);
            state.jobs.start(handle.clone());
            if !cfg!(debug_assertions) {
                updater::start(handle.clone()); // dev builds don't update themselves
            }
            // Keep the catalog fresh while the launcher stays open (new versions -> update badges / auto-update).
            let h2 = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30 * 60)).await;
                    catalog::refresh(h2.clone()).await;
                }
            });
            // Safety net: never leave an invisible app if the UI fails to report its first paint.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let state = handle.state::<AppState>();
                if !state.shown.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::warn!("UI did not report ready within 3 s, showing window anyway");
                    let _ = commands::show_main(&handle, &state);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::app_ready,
            commands::set_settings,
            commands::refresh_catalog,
            commands::auth_start,
            commands::auth_cancel,
            commands::auth_reopen,
            commands::auth_sign_out,
            commands::open_account_page,
            commands::launcher_update_check,
            commands::launcher_update_install,
            commands::install,
            commands::verify,
            commands::pause_job,
            commands::resume_job,
            commands::cancel_job,
            commands::retry_job,
            commands::prioritize_job,
            commands::clear_finished_jobs,
            commands::uninstall,
            commands::locate_install,
            commands::pick_folder,
            commands::disk_space,
            commands::open_folder,
            commands::launch,
            commands::take_pending_link,
            commands::open_news,
            commands::open_announcement,
            commands::resolve_saves,
            commands::open_item_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Penumbra Launcher");
}
