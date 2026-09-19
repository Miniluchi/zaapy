//! The Zaapy desktop application.
//!
//! All product behaviour lives in `zaapy-core`; this crate supplies the
//! operating system (see [`platform`]), the tray, the settings window and the
//! loop that ties them together.

mod ipc;
mod logging;
mod runtime;
mod tray;

use tauri::{Manager, WindowEvent};

/// Keeps the log file's writer thread alive for the lifetime of the process.
struct LogGuard(#[allow(dead_code)] Option<tracing_appender::non_blocking::WorkerGuard>);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch means the user went looking for the app they
            // forgot was already running: show them the settings page.
            tray::show_settings(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();

            let log_directory = handle.path().app_log_dir()?;
            app.manage(LogGuard(logging::init(&log_directory)));

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting Zaapy");

            let config_path = handle.path().app_config_dir()?.join("config.json");
            app.manage(runtime::Runtime::new(config_path, log_directory));

            tray::build(&handle)?;
            runtime::spawn(handle.clone());

            // The dock icon would be misleading for something that lives in the
            // menu bar and has no document of its own.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing the settings page folds Zaapy back into the tray; the
                // bridge keeps running, which is the whole point of the product.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ipc::get_config,
            ipc::set_config,
            ipc::list_windows,
            ipc::get_status,
            ipc::open_log_folder,
            ipc::request_permissions,
        ])
        .run(tauri::generate_context!())
        .expect("Zaapy failed to start");
}
