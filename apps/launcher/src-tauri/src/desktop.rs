//! Desktop integration (M8): tray icon, close-to-tray, start with Windows, notifications, `penumbra://` links.

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

/// Brings the main window to the front (from tray, deep link, second launch).
pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Penumbra Launcher", true, None::<&str>)?;
    let library = MenuItem::with_id(app, "library", "Library", true, None::<&str>)?;
    let downloads = MenuItem::with_id(app, "downloads", "Downloads", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &library, &downloads, &sep, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Penumbra Launcher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show(app),
            "library" | "downloads" => {
                show(app);
                let _ = app.emit("navigate", e.id.as_ref());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Windows toast when a download finishes while the launcher isn't in front.
pub fn notify_done(app: &AppHandle, title: &str, body: &str) {
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().notifications {
        return;
    }
    let in_front = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if in_front {
        return; // the in-app toast is enough
    }
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("notification failed: {e}");
    }
}

/// Applies the "Start with Windows" setting to the Run key.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    match r {
        Ok(()) => tracing::info!(enabled, "start with Windows updated"),
        Err(e) => tracing::warn!("start with Windows: {e}"),
    }
}

#[derive(Serialize, Clone)]
pub struct DeepLink {
    /// `open` | `install` | `play` — the UI never starts or installs anything without a click.
    pub action: String,
    pub item: String,
}

/// `penumbra://<action>/<item>` -> UI event. Unknown or malformed links are ignored.
pub fn handle_url(app: &AppHandle, url: &str) {
    let Some(rest) = url.strip_prefix("penumbra://") else { return };
    let mut parts = rest.trim_end_matches('/').splitn(2, '/');
    let (Some(action), Some(item)) = (parts.next(), parts.next()) else { return };
    let action = action.to_ascii_lowercase();
    if !["open", "install", "play"].contains(&action.as_str()) || !penumbra_core::chunk::is_valid_item_id(item) {
        tracing::warn!(%url, "ignoring deep link");
        return;
    }
    tracing::info!(%action, %item, "deep link");
    show(app);
    let link = DeepLink { action, item: item.to_string() };
    *app.state::<AppState>().pending_link.lock().unwrap() = Some(link.clone());
    let _ = app.emit("deeplink", link);
}
