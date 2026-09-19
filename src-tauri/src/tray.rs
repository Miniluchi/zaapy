//! The tray icon, which is Zaapy's only permanent presence.
//!
//! The app starts here rather than in a window: in normal use the user never
//! opens the settings page at all.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime as TauriRuntime};

use crate::runtime::Runtime;

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

    let toggle = enabled.clone();
    let mut builder = TrayIconBuilder::with_id("zaapy")
        .tooltip("Zaapy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "settings" => show_settings(app),
            "enabled" => {
                let runtime = app.state::<Runtime>();
                let mut config = runtime.config();
                config.enabled = toggle.is_checked().unwrap_or(!config.enabled);
                if let Err(error) = runtime.update_config(config) {
                    tracing::error!(%error, "could not persist the bridge switch");
                }
            }
            "quit" => app.exit(0),
            other => tracing::debug!(id = other, "unhandled tray menu item"),
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone()).icon_as_template(true);
    }
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
