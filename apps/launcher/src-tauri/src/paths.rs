//! Well-known folders (docs/05-launcher-app.md#local-files).

use std::path::PathBuf;

const APP_DIR: &str = "Penumbra Launcher";

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Roaming, small: settings.json, library.json, window state.
pub fn config_dir() -> PathBuf {
    env_dir("APPDATA").join(APP_DIR)
}

/// Local, big or regenerable: cache, logs. Uses the app identifier folder (where WebView2 keeps its data too),
/// NOT `%LOCALAPPDATA%\Penumbra Launcher`: that's the installer's default program folder (ADR-025).
pub fn local_dir() -> PathBuf {
    env_dir("LOCALAPPDATA").join("com.eclip5e.penumbra.launcher")
}

pub fn logs_dir() -> PathBuf {
    local_dir().join("logs")
}

/// Write a file atomically: temp file in the same folder, then rename over the target.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
