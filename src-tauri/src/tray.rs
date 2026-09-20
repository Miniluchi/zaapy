//! The tray icon, which is Zaapy's only permanent presence.
//!
//! The app starts here rather than in a window: in normal use the user never
//! opens the settings page at all.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime as TauriRuntime};

use crate::runtime::{self, Runtime};

/// The tray's copy of the bridge switch, kept in state so a change made in the
/// settings page can move the check mark. Without it the two surfaces drift
/// apart the moment either one is used.
pub struct BridgeToggle<R: TauriRuntime>(pub CheckMenuItem<R>);

pub fn build<R: TauriRuntime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let enabled_now = app.state::<Runtime>().config().enabled;

    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let enabled = CheckMenuItem::with_id(
        app,
        "enabled",
        "Bridge enabled",
        true,
        enabled_now,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Zaapy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &settings,
            &enabled,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    // Not the application icon: that one is a square with a background, which the
    // menu bar and the notification area both want without. Two sizes because the
    // menu bar rescales whatever it is given to 18 pt — 36 px on a Retina display
    // — while Windows builds the icon at its source size and lets the shell scale
    // it down, which it does cleanly from 32 and badly from anything larger.
    #[cfg(target_os = "macos")]
    let icon = tauri::include_image!("icons/tray-macos.png");
    #[cfg(not(target_os = "macos"))]
    let icon = tauri::include_image!("icons/tray.png");

    let toggle = enabled.clone();
    let builder = TrayIconBuilder::with_id("zaapy")
        .tooltip("Zaapy")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "settings" => show_settings(app),
            "enabled" => {
                let mut config = app.state::<Runtime>().config();
                config.enabled = toggle.is_checked().unwrap_or(!config.enabled);
                if let Err(error) = runtime::apply_config(app, config) {
                    tracing::error!(%error, "could not persist the bridge switch");
                }
            }
            "quit" => app.exit(0),
            other => tracing::debug!(id = other, "unhandled tray menu item"),
        });

    app.manage(BridgeToggle(enabled));
    builder.build(app)?;
    Ok(())
}

/// Bring the settings page up, creating nothing — the window exists from
/// startup, merely hidden.
pub fn show_settings<R: TauriRuntime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
