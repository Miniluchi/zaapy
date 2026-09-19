//! The surface the settings page talks to.

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use zaapy_core::platform::WindowRef;
use zaapy_core::Config;

use crate::runtime::{Runtime, Status};

#[tauri::command]
pub fn get_config(runtime: State<'_, Runtime>) -> Config {
    runtime.config()
}

#[tauri::command]
pub fn set_config(config: Config, runtime: State<'_, Runtime>) -> Result<Config, String> {
    runtime.update_config(config)
}

/// Every visible window on the machine, so the user can point at Ganymède and at
/// their client rather than typing executable names Zaapy would have to guess.
#[tauri::command]
pub fn list_windows() -> Vec<WindowRef> {
    zaapy_platform::list_windows()
}

#[tauri::command]
pub fn get_status(runtime: State<'_, Runtime>) -> Status {
    runtime.status()
}

/// Show the rotating log in the file manager. Opened from Rust rather than the
/// webview, so the front end needs no filesystem capability of its own.
#[tauri::command]
pub fn open_log_folder(app: AppHandle, runtime: State<'_, Runtime>) -> Result<(), String> {
    let directory = runtime.log_directory().to_path_buf();
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    app.opener()
        .open_path(directory.to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn request_permissions() {
    zaapy_platform::request_permissions();
}
