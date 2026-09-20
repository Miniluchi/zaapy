//! The macOS bridge: `NSPasteboard`, `NSWorkspace` and the Accessibility API.
//!
//! The shape follows [`win32`](super::win32) section for section, because the
//! core asks both platforms the same four questions. Where the answer differs
//! the comment says why, since almost every difference here is a workaround for
//! something macOS does not offer rather than a choice.
//!
//! Nothing in this file works until the user grants Accessibility permission,
//! with one deliberate exception: [`Windows::foreground`] reads `NSWorkspace`,
//! which needs nothing. That keeps the source gate — "was Ganymède in front when
//! this was copied?" — honest from the first launch, before any dialog.

use std::ptr::NonNull;
use std::time::Duration;

use objc2_app_kit::{
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSPasteboard,
    NSPasteboardTypeString, NSRunningApplication, NSWorkspace,
};
use objc2_application_services::{
    kAXTrustedCheckOptionPrompt, AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
    AXUIElement,
};
use objc2_core_foundation::{
    kCFBooleanFalse, kCFBooleanTrue, CFBoolean, CFDictionary, CFRetained, CFString, CFType,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGKeyCode,
};
use zaapy_core::platform::{
    ClipboardPort, InputPort, Key, Modifier, PlatformError, SendStep, WindowPort, WindowRef,
};

use crate::HostStatus;

/// How long a key stays down. The same 25 ms as the Win32 layer, for the same
/// reason: a client sampling the keyboard once per frame has to find the key
/// down at least once.
const KEY_HOLD_MS: u64 = 25;

/// Said in one place, because the settings panel and a failed send both show it.
const ACCESSIBILITY_DENIED: &str = "Zaapy needs Accessibility permission to raise the Dofus \
     window and type into it. Grant it in System Settings › Privacy & Security › Accessibility.";

// ---------------------------------------------------------------- clipboard

#[derive(Default)]
pub struct Clipboard;

fn pasteboard_text(pasteboard: &NSPasteboard) -> Option<String> {
    let text = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }?;
    Some(text.to_string())
}

impl ClipboardPort for Clipboard {
    fn sequence(&self) -> u64 {
        // The exact counterpart of `GetClipboardSequenceNumber`: a counter that
        // moves on every copy, readable without touching the content.
        NSPasteboard::generalPasteboard().changeCount() as u64
    }

    fn read_text(&self) -> Option<String> {
        pasteboard_text(&NSPasteboard::generalPasteboard())
    }

    fn clear_if_matches(&self, expected: &str) -> bool {
        // Win32 reads and empties while holding the clipboard open, so nothing
        // can slip in between the two. `NSPasteboard` has no such lock, so the
        // change counter stands in for one: if it moved while we were reading,
        // the user copied something else and the clipboard is no longer ours to
        // empty. The race is resolved towards leaving it alone, which is the
        // only direction that cannot destroy someone's copy.
        let pasteboard = NSPasteboard::generalPasteboard();
        let before = pasteboard.changeCount();

        if pasteboard_text(&pasteboard).as_deref() != Some(expected) {
            return false;
        }
        if pasteboard.changeCount() != before {
            return false;
        }

        pasteboard.clearContents();
        true
    }
}

// ------------------------------------------------------------------ windows

#[derive(Default)]
pub struct Windows;

/// Read one Accessibility attribute, owning the result.
fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let name = CFString::from_str(name);
    let mut value: *const CFType = std::ptr::null();
    let status = unsafe { element.copy_attribute_value(&name, NonNull::from(&mut value)) };
    if status != AXError::Success {
        return Err(status);
    }
    NonNull::new(value.cast_mut())
        .map(|value| unsafe { CFRetained::from_raw(value) })
        .ok_or(AXError::NoValue)
}

fn set_attribute(element: &AXUIElement, name: &str, value: bool) -> AXError {
    let name = CFString::from_str(name);
    let value = if value {
        unsafe { kCFBooleanTrue }
    } else {
        unsafe { kCFBooleanFalse }
    };
    match value {
        Some(value) => unsafe { element.set_attribute_value(&name, value) },
        None => AXError::Failure,
    }
}

/// Untrusted Accessibility is reported as a per-call error rather than a global
/// flag, and two codes mean it, so the test lives here.
fn is_denial(status: AXError) -> bool {
    matches!(status, AXError::APIDisabled | AXError::NotImplemented)
}

/// The window an application would bring forward if it were activated.
///
/// `AXFocusedWindow` answers this even for an application that is not in front.
/// `AXMainWindow` covers the one that genuinely has nothing focused — a window
/// that was just de-minimised, most often.
fn focused_window(app: &AXUIElement) -> Result<CFRetained<AXUIElement>, AXError> {
    let window = match attribute(app, "AXFocusedWindow") {
        Ok(window) => window,
        Err(status) if is_denial(status) => return Err(status),
        Err(_) => attribute(app, "AXMainWindow")?,
    };
    window
        .downcast::<AXUIElement>()
        .map_err(|_| AXError::NoValue)
}

fn window_title(pid: i32) -> Option<String> {
    let app = unsafe { AXUIElement::new_application(pid) };
    let window = focused_window(&app).ok()?;
    let title = attribute(&window, "AXTitle").ok()?;
    let title = title.downcast::<CFString>().ok()?.to_string();
    (!title.is_empty()).then_some(title)
}

/// macOS has no public, stable identifier for a single window, so the handle is
/// the process id and an application contributes exactly one row.
///
/// The core only ever compares `handle` against [`Windows::foreground`] to
/// confirm that the window it asked for came forward — and on macOS the
/// frontmost window *is* the focused window of the frontmost application, so a
/// pid answers that question exactly, and cannot go stale mid-send the way a
/// recycled window id could. What it costs is a multi-window application
/// appearing once in the picker. Dofus runs one process per client, so
/// multi-account selection is untouched.
fn describe(app: &NSRunningApplication) -> Option<WindowRef> {
    // Regular applications are the ones with a Dock icon and windows a user can
    // look at: the stand-in for Win32's "visible, and has a title". It drops
    // daemons, launch agents and menu-bar-only apps, Zaapy itself included.
    if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
        return None;
    }
    let pid = app.processIdentifier();
    if pid <= 0 {
        return None;
    }

    let name = app.localizedName().map(|name| name.to_string());
    // The executable's file name, to match Win32's `Dofus.exe`. The localised
    // application name is the fallback, and on a Mac the two rarely differ.
    let process = app
        .executableURL()
        .and_then(|url| url.lastPathComponent())
        .map(|component| component.to_string())
        .or_else(|| name.clone())?;

    Some(WindowRef {
        handle: pid as u64,
        pid: pid as u32,
        process,
        // Without Accessibility the title is unreadable. Falling back to the
        // application name keeps the picker populated, so the panel asks for the
        // permission instead of looking broken.
        title: window_title(pid).or(name).unwrap_or_default(),
    })
}

impl WindowPort for Windows {
    fn foreground(&self) -> Option<WindowRef> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        describe(&app)
    }

    fn list_visible(&self) -> Vec<WindowRef> {
        // Documented as safe to call from a background thread and returned
        // atomically, which the bridge relies on: it runs on its own thread.
        NSWorkspace::sharedWorkspace()
            .runningApplications()
            .iter()
            .filter_map(|app| describe(&app))
            .collect()
    }

    fn focus(&self, window: &WindowRef) -> Result<(), PlatformError> {
        let pid = window.pid as i32;
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return Err(PlatformError::FocusRejected {
                detail: "the application is no longer running".to_string(),
            });
        };

        let element = unsafe { AXUIElement::new_application(pid) };
        let focused = match focused_window(&element) {
            Ok(window) => Some(window),
            // Said here rather than letting the core spend its focus timeout
            // waiting for a window that was never going to move.
            Err(status) if is_denial(status) => {
                return Err(PlatformError::PermissionDenied {
                    detail: ACCESSIBILITY_DENIED.to_string(),
                })
            }
            // An application with no window yet is not an error: activating it
            // may open one, and the core confirms the result either way.
            Err(_) => None,
        };

        if let Some(focused) = &focused {
            // The counterpart of `IsIconic` + `SW_RESTORE`: a minimised window
            // cannot be raised, only un-minimised.
            set_attribute(focused, "AXMinimized", false);
            let _ = unsafe { focused.perform_action(&CFString::from_str("AXRaise")) };
        }
        set_attribute(&element, "AXFrontmost", true);

        // Raised through Accessibility *and* activated through NSWorkspace.
        // Since macOS 14 the system may ignore an activation request from an
        // application that is not itself active, and Zaapy runs as an accessory
        // with no window of its own; `AXRaise` is the path that works without
        // that cooperation, while `activate` is the one that moves the menu bar.
        // Neither is dependable alone.
        app.activateWithOptions(NSApplicationActivationOptions::empty());

        // Like the Win32 layer, this reports only that the request went out.
        // Whether the window actually came forward is the core's to confirm.
        Ok(())
    }
}

// -------------------------------------------------------------------- input

#[derive(Default)]
pub struct Input;

/// ANSI virtual key codes.
///
/// These name a *position* on the keyboard rather than a character — on AZERTY,
/// `0x00` is the key that prints `q`, not `a`. That is the same trap the Win32
/// layer documents for scan codes, and it is the reason the send sequence pastes
/// instead of typing the command out. `v` happens to sit in the same place on
/// both layouts, so Command-V survives either way.
fn virtual_key(key: Key) -> Option<CGKeyCode> {
    Some(match key {
        Key::Enter => 0x24,
        Key::Escape => 0x35,
        Key::Tab => 0x30,
        Key::Space => 0x31,
        Key::Backspace => 0x33,
        Key::Delete => 0x75,
        Key::Char(c) => match c.to_ascii_lowercase() {
            'a' => 0x00,
            'b' => 0x0B,
            'c' => 0x08,
            'd' => 0x02,
            'e' => 0x0E,
            'f' => 0x03,
            'g' => 0x05,
            'h' => 0x04,
            'i' => 0x22,
            'j' => 0x26,
            'k' => 0x28,
            'l' => 0x25,
            'm' => 0x2E,
            'n' => 0x2D,
            'o' => 0x1F,
            'p' => 0x23,
            'q' => 0x0C,
            'r' => 0x0F,
            's' => 0x01,
            't' => 0x11,
            'u' => 0x20,
            'v' => 0x09,
            'w' => 0x0D,
            'x' => 0x07,
            'y' => 0x10,
            'z' => 0x06,
            '0' => 0x1D,
            '1' => 0x12,
            '2' => 0x13,
            '3' => 0x14,
            '4' => 0x15,
            '5' => 0x17,
            '6' => 0x16,
            '7' => 0x1A,
            '8' => 0x1C,
            '9' => 0x19,
            _ => return None,
        },
    })
}

/// The key a modifier presses, and the flag it raises. macOS needs both.
fn modifier_key(modifier: Modifier) -> (CGKeyCode, CGEventFlags) {
    match modifier {
        // On macOS the portable "primary" modifier is Command, and there is no
        // separate meta key for `Meta` to land on.
        Modifier::Primary | Modifier::Meta => (0x37, CGEventFlags::MaskCommand),
        Modifier::Ctrl => (0x3B, CGEventFlags::MaskControl),
        Modifier::Alt => (0x3A, CGEventFlags::MaskAlternate),
        Modifier::Shift => (0x38, CGEventFlags::MaskShift),
    }
}

fn post(
    source: &CGEventSource,
    key: CGKeyCode,
    down: bool,
    flags: CGEventFlags,
) -> Result<(), PlatformError> {
    let event = CGEvent::new_keyboard_event(Some(source), key, down).ok_or_else(|| {
        PlatformError::InputRejected {
            detail: "the system refused to build a keyboard event".to_string(),
        }
    })?;

    // A synthesised modifier key-down does not change the flags the system
    // stamps onto the events that follow it — those are read from the real
    // hardware. Setting them by hand is what makes Dofus see Command-V rather
    // than a bare `v`, and posting the modifier keys as well is what makes it
    // see the Command press at all. Both, or the chord is not a chord.
    CGEvent::set_flags(Some(&event), flags);

    // The HID tap is the lowest point an event can be injected, and the macOS
    // counterpart of filling in a scan code on Windows: a game reading below the
    // session layer never sees anything posted higher up.
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    Ok(())
}

fn hold() {
    std::thread::sleep(Duration::from_millis(KEY_HOLD_MS));
}

/// Press a key, hold it, release it — each posted on its own. Sent together they
/// reach a client polling once per frame as a key that was never down.
fn tap(source: &CGEventSource, key: CGKeyCode, flags: CGEventFlags) -> Result<(), PlatformError> {
    post(source, key, true, flags)?;
    hold();
    post(source, key, false, flags)
}

impl InputPort for Input {
    fn send(&self, steps: &[SendStep]) -> Result<(), PlatformError> {
        // `CGEventPost` returns nothing and drops every event silently when the
        // process is not a trusted Accessibility client. Asking up front is what
        // turns a session of nothing happening into one sentence the user can
        // act on — the same trap the Win32 layer heads off for UIPI.
        if !trusted() {
            return Err(PlatformError::PermissionDenied {
                detail: ACCESSIBILITY_DENIED.to_string(),
            });
        }

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok_or_else(|| {
            PlatformError::InputRejected {
                detail: "the system refused to open an event source".to_string(),
            }
        })?;

        for step in steps {
            match step {
                SendStep::Delay { ms } => std::thread::sleep(Duration::from_millis(*ms)),
                SendStep::Key { key, modifiers } => {
                    let code = virtual_key(*key).ok_or_else(|| PlatformError::InputRejected {
                        detail: format!("`{key}` has no macOS key code"),
                    })?;
                    let held: Vec<(CGKeyCode, CGEventFlags)> =
                        modifiers.iter().copied().map(modifier_key).collect();
                    let all = held
                        .iter()
                        .fold(CGEventFlags::empty(), |all, (_, flag)| all | *flag);

                    // The modifiers go down first, and settle, so the game has
                    // registered them before the key arrives. Each one carries
                    // the flags raised so far, the way real hardware reports it.
                    let mut raised = CGEventFlags::empty();
                    let mut pressed = Ok(());
                    for (code, flag) in &held {
                        raised |= *flag;
                        pressed = pressed.and_then(|()| post(&source, *code, true, raised));
                    }
                    if !held.is_empty() && pressed.is_ok() {
                        hold();
                    }

                    let tapped = pressed.and_then(|()| tap(&source, code, all));

                    // Let go of the modifiers whatever happened to the key: a
                    // Command left down would follow the user around for the
                    // rest of their session. Released in reverse, so the chord
                    // unwinds the way a human would let go of it.
                    let mut lowered = all;
                    for (code, flag) in held.iter().rev() {
                        lowered &= !*flag;
                        let _ = post(&source, *code, false, lowered);
                    }
                    tapped?;
                }
            }
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- status

fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// `target_pid` goes unused: macOS has no equivalent of Windows' privilege
/// isolation, where a more privileged target swallows keystrokes without a word.
/// Either Accessibility is granted and every application is reachable, or it is
/// not and none of them are.
pub fn host_status(_target_pid: Option<u32>) -> HostStatus {
    let trusted = trusted();
    HostStatus {
        input_permitted: trusted,
        privilege_warning: (!trusted).then(|| ACCESSIBILITY_DENIED.to_string()),
        can_request_permission: !trusted,
    }
}

pub fn request_permissions() {
    let prompt = unsafe { kAXTrustedCheckOptionPrompt };
    let Some(yes) = (unsafe { kCFBooleanTrue }) else {
        return;
    };
    let options = CFDictionary::<CFString, CFBoolean>::from_slices(&[prompt], &[yes]);
    // The key and value types are a compile-time tag on what is one untyped
    // Core Foundation object, which is what the Accessibility binding asks for.
    let options: CFRetained<CFDictionary> = unsafe { CFRetained::cast_unchecked(options) };

    // macOS shows the prompt once per binary and remembers the answer; after
    // that this returns quietly and the user has to open System Settings
    // themselves. The settings panel keeps showing the warning until they do.
    if unsafe { AXIsProcessTrustedWithOptions(Some(&options)) } {
        tracing::info!("accessibility permission is already granted");
    } else {
        tracing::info!(
            "accessibility permission requested; macOS prompts once per binary, after which it \
             has to be granted from System Settings"
        );
    }
}
