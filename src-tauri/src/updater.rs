//! Updates are offered, never applied on their own: Zaapy checks once at startup
//! and adds a tray item, and only a click on it installs and restarts — never in
//! the middle of a guide.

use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Runtime as TauriRuntime};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use crate::tray::TrayMenu;

pub const MENU_ID: &str = "update";

pub fn check<R: TauriRuntime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let update = match app.updater() {
            Ok(updater) => updater.check().await,
            Err(error) => Err(error),
        };
        match update {
            Ok(Some(update)) => offer(&app, &update.version),
            Ok(None) => tracing::info!("Zaapy is up to date"),
            // Offline, GitHub down: nothing the user needs to hear about.
            Err(error) => tracing::warn!(%error, "could not check for updates"),
        }
    });
}

fn offer<R: TauriRuntime>(app: &AppHandle<R>, version: &str) {
    tracing::info!(version, "update available");
    let label = format!("Update to {version}…");
    let added = MenuItem::with_id(app, MENU_ID, label, true, None::<&str>)
        .and_then(|item| app.state::<TrayMenu<R>>().0.insert(&item, 0));
    if let Err(error) = added {
        tracing::error!(%error, "could not add the update item to the tray");
        return;
    }
    notify(
        app,
        &format!("Zaapy {version} is available. Install it from the tray menu."),
    );
}

pub fn install<R: TauriRuntime>(app: &AppHandle<R>) {
    let item = app.state::<TrayMenu<R>>().0.get(MENU_ID);
    let item = item.as_ref().and_then(|item| item.as_menuitem());
    // A second click while the download runs would start a second one.
    if let Some(item) = item {
        let _ = item.set_enabled(false);
    }
    let item = item.cloned();

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Checked again rather than kept from startup: the app may have been up
        // for days, and a newer release may have come out since.
        let result = async {
            match app.updater()?.check().await? {
                Some(update) => update
                    .download_and_install(|_, _| {}, || {})
                    .await
                    .map(|_| true),
                None => Ok(false),
            }
        }
        .await;
        match result {
            Ok(true) => app.restart(),
            Ok(false) => tracing::info!("no update left to install"),
            Err(error) => {
                tracing::error!(%error, "update failed");
                notify(&app, &format!("The update failed: {error}"));
                if let Some(item) = item {
                    let _ = item.set_enabled(true);
                }
            }
        }
    });
}

fn notify<R: TauriRuntime>(app: &AppHandle<R>, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title("Zaapy")
        .body(body)
        .show();
}
