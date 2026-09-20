//! Application state and the loop that drives the bridge.
//!
//! The core's [`Bridge`] is single-threaded and synchronous by design, so the
//! host's job is small: call it every 100 ms, write what happened to the log
//! file, and notify the user when — and only when — a command could not be
//! delivered.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime as TauriRuntime};
use tauri_plugin_notification::NotificationExt;
use zaapy_core::bridge::{self, Outcome, TickReport};
use zaapy_core::platform::WindowRef;
use zaapy_core::{Bridge, Config, TICK_INTERVAL_MS};

use crate::tray::BridgeToggle;

/// Broadcast to the settings page whenever the configuration changes, whoever
/// changed it.
pub const CONFIG_CHANGED: &str = "config-changed";

/// Everything the settings page needs to tell the user where they stand.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    /// Whether a source app and a target have both been picked.
    pub configured: bool,
    /// The window a command would be sent to right now, if it resolves.
    pub target: Option<WindowRef>,
    pub target_error: Option<String>,
    pub host: zaapy_platform::HostStatus,
}

pub struct Runtime {
    config: Mutex<Config>,
    /// Held for the whole duration of a send, including the focus wait — which
    /// is why the window picker reads the OS directly instead of going through it.
    bridge: Mutex<Bridge>,
    config_path: PathBuf,
    log_directory: PathBuf,
}

impl Runtime {
    pub fn new(config_path: PathBuf, log_directory: PathBuf) -> Self {
        let config = read_config(&config_path);
        let bridge = Bridge::new(zaapy_platform::ports(), config.clone());

        Self {
            config: Mutex::new(bridge.config().clone()),
            bridge: Mutex::new(bridge),
            config_path,
            log_directory,
        }
    }

    /// Where the rotating log lives. With the in-app journal gone, this is the
    /// record of what the bridge did, and the settings page links straight to it.
    pub fn log_directory(&self) -> &Path {
        &self.log_directory
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    /// Apply a configuration change everywhere at once: the running bridge, the
    /// in-memory copy the UI reads, and the file on disk.
    pub fn update_config(&self, config: Config) -> Result<Config, String> {
        let sanitised = config.sanitised();
        self.bridge.lock().unwrap().set_config(sanitised.clone());
        *self.config.lock().unwrap() = sanitised.clone();
        write_config(&self.config_path, &sanitised)?;
        tracing::info!("configuration updated");
        Ok(sanitised)
    }

    pub fn status(&self) -> Status {
        let config = self.config();
        let resolution = bridge::resolve_target(&config.target, zaapy_platform::list_windows());
        let (target, target_error) = match resolution {
            Ok(window) => (Some(window), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Status {
            enabled: config.enabled,
            configured: config.is_configured(),
            host: zaapy_platform::host_status(target.as_ref().map(|window| window.pid)),
            target,
            target_error,
        }
    }

    fn tick(&self) -> Option<TickReport> {
        self.bridge.lock().unwrap().tick()
    }
}

/// The one way a configuration change reaches the whole application: the running
/// bridge and the file on disk, the tray's check mark, and the settings page.
///
/// Both surfaces write through here, which is what keeps the bridge switch from
/// reading one thing in the tray menu and another in the panel.
pub fn apply_config<R: TauriRuntime>(app: &AppHandle<R>, config: Config) -> Result<Config, String> {
    let saved = app.state::<Runtime>().update_config(config)?;

    // Menu items may only be touched from the main thread on macOS, and a command
    // arriving from the webview does not run on it.
    let handle = app.clone();
    let checked = saved.enabled;
    let _ = app.run_on_main_thread(move || {
        if let Some(toggle) = handle.try_state::<BridgeToggle<R>>() {
            let _ = toggle.0.set_checked(checked);
        }
    });

    let _ = app.emit(CONFIG_CHANGED, &saved);
    Ok(saved)
}

/// Start the 100 ms loop. Runs until the process exits.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("zaapy-bridge".to_string())
        .spawn(move || {
            let interval = Duration::from_millis(TICK_INTERVAL_MS);
            loop {
                std::thread::sleep(interval);

                let Some(report) = tauri::Manager::state::<Runtime>(&app).tick() else {
                    continue;
                };
                handle(&app, report);
            }
        })
        .expect("the bridge thread must start");
}

fn handle(app: &AppHandle, report: TickReport) {
    match &report.outcome {
        Outcome::Sent {
            command,
            target_title,
        } => {
            // Deliberately not "delivered": all we know is that the system took
            // the keystrokes. Whether the game acted on them is unobservable.
            tracing::info!(%command, target = %target_title, "keystrokes sent to the target")
        }
        Outcome::Failed { command, error } => {
            tracing::warn!(%command, %error, "command could not be delivered")
        }
        Outcome::Ignored { reason } => tracing::debug!(?reason, "clipboard change ignored"),
    }

    // Only failures interrupt the user. A working bridge is a silent one.
    if let Outcome::Failed { error, .. } = &report.outcome {
        let _ = app
            .notification()
            .builder()
            .title("Zaapy")
            .body(error.to_string())
            .show();
    }
}

fn read_config(path: &Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Config>(&contents) {
            Ok(config) => config,
            Err(error) => {
                // Never let a damaged file stop the app from starting; the user
                // can re-pick their windows in seconds.
                tracing::error!(%error, "configuration file is unreadable, falling back to defaults");
                Config::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(error) => {
            tracing::error!(%error, "configuration file could not be read");
            Config::default()
        }
    }
}

fn write_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let encoded = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    std::fs::write(path, encoded).map_err(|error| error.to_string())
}
