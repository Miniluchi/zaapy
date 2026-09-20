//! Operating-system implementations of the core's ports.
//!
//! Nothing in here makes a decision about what Zaapy should do — it only
//! translates the core's four traits into system calls. Which module is
//! compiled is decided here and nowhere else.
//!
//! Kept apart from the Tauri application on purpose: with no dependency on
//! `tauri-build`, the Windows code can be type-checked from any machine with
//! `cargo check --target x86_64-pc-windows-msvc`, which is how it gets reviewed
//! before it ever reaches a PC.

use std::time::{SystemTime, UNIX_EPOCH};

use zaapy_core::platform::{Clock, Ports};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod win32;

#[cfg(windows)]
use win32 as sys;

#[cfg(target_os = "macos")]
use macos as sys;

#[cfg(not(any(windows, target_os = "macos")))]
use unsupported as sys;

/// Wall-clock time, in milliseconds since the epoch.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// What the app needs to warn the user about before they hit a silent failure.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// `false` on macOS until Accessibility is granted; always `true` on Windows.
    pub input_permitted: bool,
    /// Set when the target runs with privileges Zaapy lacks, which makes
    /// keystroke injection fail without any error the OS will tell us about.
    pub privilege_warning: Option<String>,
    /// Whether there is anything the user can actually grant. False on Windows,
    /// where privilege is not something a dialog can fix, so the settings page
    /// does not offer a button that would do nothing.
    pub can_request_permission: bool,
}

pub fn ports() -> Ports {
    Ports {
        clipboard: Box::new(sys::Clipboard),
        windows: Box::new(sys::Windows),
        input: Box::new(sys::Input),
        clock: Box::new(SystemClock),
    }
}

/// Inspect the host for the traps that produce a bridge that looks healthy and
/// does nothing: missing macOS permission, or an elevated target on Windows.
pub fn host_status(target_pid: Option<u32>) -> HostStatus {
    sys::host_status(target_pid)
}

/// Ask the OS to walk the user through granting what Zaapy needs. No-op where
/// there is nothing to grant.
pub fn request_permissions() {
    sys::request_permissions();
}

/// The visible windows on the machine, for the settings page's live picker.
///
/// Deliberately not routed through the bridge: the picker refreshes while a
/// send may be in flight, and it has no business waiting on that lock.
pub fn list_windows() -> Vec<zaapy_core::platform::WindowRef> {
    use zaapy_core::platform::WindowPort;
    sys::Windows.list_visible()
}
