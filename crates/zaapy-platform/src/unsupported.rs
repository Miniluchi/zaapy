//! The fallback for every platform whose bridge has not been written yet.
//!
//! Today that means macOS, which is next in line: it will use
//! `NSPasteboard.changeCount` for the clipboard, `NSWorkspace` and the
//! Accessibility API (`AXUIElement`) for window titles and raising, and
//! `CGEvent` for keystrokes — none of which work until the user grants
//! Accessibility permission, so [`host_status`] is where that will be reported.
//!
//! It also covers Linux, which keeps the crate compiling on a CI runner that has
//! neither Windows nor a Mac.
//!
//! Everything here fails honestly rather than silently: the app runs, the
//! settings page works, and the journal says exactly why nothing was sent.

use zaapy_core::platform::{
    ClipboardPort, InputPort, PlatformError, SendStep, WindowPort, WindowRef,
};

use crate::HostStatus;

const UNSUPPORTED: &str = "the bridge is not implemented on this platform yet";

pub struct Clipboard;

impl ClipboardPort for Clipboard {
    fn sequence(&self) -> u64 {
        0
    }

    fn read_text(&self) -> Option<String> {
        None
    }

    fn clear_if_matches(&self, _expected: &str) -> bool {
        false
    }
}

pub struct Windows;

impl WindowPort for Windows {
    fn foreground(&self) -> Option<WindowRef> {
        None
    }

    fn list_visible(&self) -> Vec<WindowRef> {
        Vec::new()
    }

    fn focus(&self, _window: &WindowRef) -> Result<(), PlatformError> {
        Err(PlatformError::Other {
            detail: UNSUPPORTED.to_string(),
        })
    }
}

pub struct Input;

impl InputPort for Input {
    fn send(&self, _steps: &[SendStep]) -> Result<(), PlatformError> {
        Err(PlatformError::Other {
            detail: UNSUPPORTED.to_string(),
        })
    }
}

pub fn host_status(_target_pid: Option<u32>) -> HostStatus {
    HostStatus {
        input_permitted: false,
        privilege_warning: Some(UNSUPPORTED.to_string()),
        // Nothing to grant until the macOS bridge exists.
        can_request_permission: false,
    }
}

pub fn request_permissions() {}
