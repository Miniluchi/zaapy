//! Windows implementation of the core ports.
//!
//! Four Win32 quirks shape this file:
//!
//! * The clipboard is a *shared, lockable* resource — `OpenClipboard` fails
//!   while another process holds it, so every access retries briefly.
//! * A background process cannot simply call `SetForegroundWindow`; Windows'
//!   foreground lock silently ignores it. The `AttachThreadInput` dance below is
//!   the established workaround, and the core still confirms the result.
//! * `SendInput` targets whatever has focus and fails *silently* when the target
//!   is more privileged than we are (UIPI), which is why elevation is reported
//!   up front rather than diagnosed after the fact.
//! * A synthesised keystroke only looks real to a game if it carries a scan code
//!   and stays down long enough to be sampled — see [`key_event`] and
//!   [`KEY_HOLD_MS`]. A virtual key code alone drives every desktop application
//!   and no game at all.

use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HGLOBAL, HWND, LPARAM};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_BACK, VK_CONTROL,
    VK_DELETE, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow,
    SW_RESTORE,
};
use zaapy_core::platform::{
    ClipboardPort, InputPort, Key, Modifier, PlatformError, SendStep, WindowPort, WindowRef,
};

use crate::HostStatus;

/// The clipboard is contended; a few short retries beat failing a travel.
const CLIPBOARD_ATTEMPTS: u32 = 5;
const CLIPBOARD_RETRY_MS: u64 = 10;

/// How long a key stays down, and how long a modifier settles before the key it
/// qualifies. A client that samples its input once per frame never sees a press
/// that goes down and up inside the same frame.
const KEY_HOLD_MS: u64 = 25;

// ---------------------------------------------------------------- clipboard

#[derive(Default)]
pub struct Clipboard;

/// Run `body` while holding the clipboard, retrying while another process has it.
fn with_clipboard<T>(body: impl Fn() -> T) -> Option<T> {
    for attempt in 0..CLIPBOARD_ATTEMPTS {
        let opened = unsafe { OpenClipboard(None) };
        if opened.is_ok() {
            let result = body();
            unsafe {
                let _ = CloseClipboard();
            }
            return Some(result);
        }
        if attempt + 1 < CLIPBOARD_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(CLIPBOARD_RETRY_MS));
        }
    }
    tracing::debug!("clipboard stayed locked by another process");
    None
}

/// Read the clipboard's unicode text. Must be called with the clipboard open.
fn read_unicode_text() -> Option<String> {
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
        let global = HGLOBAL(handle.0);
        let pointer = GlobalLock(global) as *const u16;
        if pointer.is_null() {
            return None;
        }

        let mut length = 0usize;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let text = std::ffi::OsString::from_wide(std::slice::from_raw_parts(pointer, length))
            .to_string_lossy()
            .into_owned();

        let _ = GlobalUnlock(global);
        Some(text)
    }
}

impl ClipboardPort for Clipboard {
    fn sequence(&self) -> u64 {
        // Cheap and lock-free: this is what lets the bridge poll at 100 ms
        // without ever touching the content itself.
        unsafe { GetClipboardSequenceNumber() as u64 }
    }

    fn read_text(&self) -> Option<String> {
        with_clipboard(read_unicode_text).flatten()
    }
}

// ------------------------------------------------------------------ windows

#[derive(Default)]
pub struct Windows;

fn window_title(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let written = GetWindowTextW(hwnd, &mut buffer);
        std::ffi::OsString::from_wide(&buffer[..written as usize])
            .to_string_lossy()
            .into_owned()
    }
}

/// The executable's file name, without its path — `Dofus.exe`.
fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let query = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        query.ok()?;

        let full = std::ffi::OsString::from_wide(&buffer[..size as usize])
            .to_string_lossy()
            .into_owned();
        full.rsplit(['\\', '/']).next().map(str::to_string)
    }
}

fn describe(hwnd: HWND) -> Option<WindowRef> {
    let title = window_title(hwnd);
    if title.is_empty() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    Some(WindowRef {
        handle: hwnd.0 as usize as u64,
        pid,
        process: process_name(pid).unwrap_or_default(),
        title,
    })
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        if let Some(window) = describe(hwnd) {
            let collected = unsafe { &mut *(lparam.0 as *mut Vec<WindowRef>) };
            collected.push(window);
        }
    }
    true.into()
}

fn to_hwnd(handle: u64) -> HWND {
    HWND(handle as usize as *mut c_void)
}

impl WindowPort for Windows {
    fn foreground(&self) -> Option<WindowRef> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return None;
        }
        describe(hwnd)
    }

    fn list_visible(&self) -> Vec<WindowRef> {
        let mut collected: Vec<WindowRef> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(collect),
                LPARAM(&mut collected as *mut Vec<WindowRef> as isize),
            );
        }
        collected
    }

    fn focus(&self, window: &WindowRef) -> Result<(), PlatformError> {
        let hwnd = to_hwnd(window.handle);
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(PlatformError::FocusRejected {
                    detail: "the window no longer exists".to_string(),
                });
            }
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            // Borrow the current foreground thread's input queue: without this,
            // Windows quietly refuses to let a background process steal focus.
            let foreground = GetForegroundWindow();
            let foreground_thread = GetWindowThreadProcessId(foreground, None);
            let our_thread = GetCurrentThreadId();
            let attached = foreground_thread != 0
                && foreground_thread != our_thread
                && AttachThreadInput(our_thread, foreground_thread, true).as_bool();

            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);

            if attached {
                let _ = AttachThreadInput(our_thread, foreground_thread, false);
            }
        }
        // Deliberately not reporting success or failure here: the return value of
        // SetForegroundWindow lies often enough that the core confirms instead.
        Ok(())
    }
}

// -------------------------------------------------------------------- input

#[derive(Default)]
pub struct Input;

fn virtual_key(key: Key) -> Option<VIRTUAL_KEY> {
    Some(match key {
        Key::Enter => VK_RETURN,
        Key::Escape => VK_ESCAPE,
        Key::Tab => VK_TAB,
        Key::Space => VK_SPACE,
        Key::Backspace => VK_BACK,
        Key::Delete => VK_DELETE,
        Key::Char(c) => {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_alphanumeric() {
                VIRTUAL_KEY(upper as u16)
            } else {
                return None;
            }
        }
    })
}

fn modifier_key(modifier: Modifier) -> VIRTUAL_KEY {
    match modifier {
        // On Windows the portable "primary" modifier is Control.
        Modifier::Primary | Modifier::Ctrl => VK_CONTROL,
        Modifier::Alt => VK_MENU,
        Modifier::Shift => VK_SHIFT,
        Modifier::Meta => VK_LWIN,
    }
}

/// Keys Windows prefixes with `E0` on the wire. `MapVirtualKeyW` reports the bare
/// scan code, so the flag has to be set by hand.
fn is_extended(key: VIRTUAL_KEY) -> bool {
    matches!(key, VK_DELETE | VK_LWIN)
}

fn key_event(key: VIRTUAL_KEY, up: bool) -> INPUT {
    // A real keyboard reports a scan code, and a game identifies the physical key
    // from it rather than from the virtual key — Raw Input carries nothing else.
    // Leaving `wScan` at zero is what makes an injected keystroke drive every
    // desktop application and no game at all. `wVk` stays filled alongside it so
    // that consumers reading window messages see the same event.
    //
    // The mapping uses the calling thread's keyboard layout, which is the user's
    // own — the same one the game reads the scan code back with.
    let scan = unsafe { MapVirtualKeyW(key.0 as u32, MAPVK_VK_TO_VSC) } as u16;

    let mut flags = KEYBD_EVENT_FLAGS::default();
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended(key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn hold() {
    std::thread::sleep(Duration::from_millis(KEY_HOLD_MS));
}

/// Press a key, hold it, release it — each as its own batch. Sent together they
/// reach a client polling once per frame as a key that was never down.
fn tap(key: VIRTUAL_KEY) -> Result<(), PlatformError> {
    dispatch(&[key_event(key, false)])?;
    hold();
    dispatch(&[key_event(key, true)])
}

fn dispatch(events: &[INPUT]) -> Result<(), PlatformError> {
    let sent = unsafe { SendInput(events, size_of::<INPUT>() as i32) };
    if sent as usize != events.len() {
        return Err(PlatformError::InputRejected {
            detail: "the system accepted no keystrokes; Dofus may be running as \
                     administrator while Zaapy is not"
                .to_string(),
        });
    }
    Ok(())
}

impl InputPort for Input {
    fn send(&self, steps: &[SendStep]) -> Result<(), PlatformError> {
        for step in steps {
            match step {
                SendStep::Delay { ms } => std::thread::sleep(Duration::from_millis(*ms)),
                SendStep::Key { key, modifiers } => {
                    let key = virtual_key(*key).ok_or_else(|| PlatformError::InputRejected {
                        detail: format!("`{key}` has no Windows key code"),
                    })?;
                    let held: Vec<VIRTUAL_KEY> =
                        modifiers.iter().copied().map(modifier_key).collect();

                    let press: Vec<INPUT> = held.iter().map(|m| key_event(*m, false)).collect();
                    // Released in reverse so the chord unwinds the way a human
                    // would let go of it.
                    let release: Vec<INPUT> =
                        held.iter().rev().map(|m| key_event(*m, true)).collect();

                    // The modifiers go down in their own batch, and settle, so
                    // that the game has registered them before the key arrives.
                    if !press.is_empty() {
                        dispatch(&press)?;
                        hold();
                    }

                    let tapped = tap(key);

                    // Let go of the modifiers whatever happened to the key: a
                    // Ctrl left down would follow the user around for the rest
                    // of their session.
                    if !release.is_empty() {
                        let _ = dispatch(&release);
                    }
                    tapped?;
                }
            }
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- status

fn is_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;

        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let query = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        query.ok()?;

        Some(elevation.TokenIsElevated != 0)
    }
}

fn process_is_elevated(pid: u32) -> Option<bool> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let elevated = is_elevated(process);
        let _ = CloseHandle(process);
        elevated
    }
}

pub fn host_status(target_pid: Option<u32>) -> HostStatus {
    let ours = unsafe { is_elevated(GetCurrentProcess()) }.unwrap_or(false);
    let privilege_warning = target_pid
        .and_then(process_is_elevated)
        .filter(|target_elevated| *target_elevated && !ours)
        .map(|_| {
            "Dofus is running as administrator and Zaapy is not, so Windows will \
             discard its keystrokes. Restart Zaapy as administrator."
                .to_string()
        });

    HostStatus {
        input_permitted: true,
        privilege_warning,
        can_request_permission: false,
    }
}

/// Windows grants input injection to any process at the same privilege level;
/// there is nothing to ask for.
pub fn request_permissions() {}
