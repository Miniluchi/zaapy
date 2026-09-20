//! The contract every operating system must fulfil.
//!
//! Everything genuinely platform-specific — watching the clipboard, knowing
//! which window is in front, raising a window owned by another process,
//! synthesising keystrokes — lives behind these four traits. The bridge never
//! calls an OS API directly, which is what makes it testable anywhere.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A top-level window belonging to some process on the machine.
///
/// `handle` is only meaningful within a single run of the target application
/// (an HWND does not survive a game restart), so it is never persisted — see
/// [`crate::config::TargetSelector`] for how a target is remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowRef {
    pub handle: u64,
    pub pid: u32,
    /// Executable file name, without its directory (`Dofus.exe`, `Ganymede`).
    pub process: String,
    pub title: String,
}

impl WindowRef {
    /// Case-insensitive comparison of the executable name, which is how both
    /// the source guard and the target selector identify an application.
    pub fn process_matches(&self, name: &str) -> bool {
        self.process.eq_ignore_ascii_case(name)
    }
}

/// A modifier as the user configures it, not as the OS knows it.
///
/// [`Modifier::Primary`] is the whole point of this enum: it means "Ctrl on
/// Windows, Command on macOS", so the default send sequence stays portable and
/// the core keeps no knowledge of the platform it runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Primary,
    Ctrl,
    Alt,
    Shift,
    Meta,
}

/// A key in the send sequence. Serialised as a plain string (`"enter"`, `"v"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    /// A single printable character, lowercased on parse.
    Char(char),
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Enter => f.write_str("enter"),
            Key::Escape => f.write_str("escape"),
            Key::Tab => f.write_str("tab"),
            Key::Space => f.write_str("space"),
            Key::Backspace => f.write_str("backspace"),
            Key::Delete => f.write_str("delete"),
            Key::Char(c) => write!(f, "{c}"),
        }
    }
}

impl FromStr for Key {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "enter" | "return" => Ok(Key::Enter),
            "escape" | "esc" => Ok(Key::Escape),
            "tab" => Ok(Key::Tab),
            "space" => Ok(Key::Space),
            "backspace" => Ok(Key::Backspace),
            "delete" | "del" => Ok(Key::Delete),
            other => {
                let mut chars = other.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphanumeric() => Ok(Key::Char(c)),
                    _ => Err(format!("unknown key `{s}`")),
                }
            }
        }
    }
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Key::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

/// One step of the configurable send sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SendStep {
    Key {
        key: Key,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    /// Breathing room between keystrokes; the game needs a moment to open its
    /// chat before it will accept a paste.
    Delay { ms: u64 },
}

impl SendStep {
    pub fn key(key: Key) -> Self {
        SendStep::Key {
            key,
            modifiers: Vec::new(),
        }
    }

    pub fn chord(modifiers: Vec<Modifier>, key: Key) -> Self {
        SendStep::Key { key, modifiers }
    }

    pub fn delay(ms: u64) -> Self {
        SendStep::Delay { ms }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlatformError {
    /// The OS refused to raise the window, or the window vanished mid-flight.
    #[error("the window could not be brought to the foreground: {detail}")]
    FocusRejected { detail: String },
    /// Keystroke injection failed. On Windows this is usually UIPI: the target
    /// runs elevated and we do not.
    #[error("keystrokes could not be delivered: {detail}")]
    InputRejected { detail: String },
    /// The OS denied us the access this operation needs (macOS Accessibility).
    #[error("permission denied: {detail}")]
    PermissionDenied { detail: String },
    #[error("{detail}")]
    Other { detail: String },
}

/// Watching the system clipboard.
///
/// Read-only on purpose: the clipboard is the user's, and the command they
/// copied stays there for them to paste by hand whatever Zaapy does with it.
pub trait ClipboardPort: Send {
    /// A counter that changes whenever the clipboard content changes. Cheap to
    /// call; it lets the bridge poll without ever reading the actual content.
    fn sequence(&self) -> u64;

    /// The clipboard as text, or `None` when it holds something else entirely
    /// (an image, a file list) or cannot be read right now.
    fn read_text(&self) -> Option<String>;
}

/// Enumerating and raising windows.
pub trait WindowPort: Send {
    fn foreground(&self) -> Option<WindowRef>;
    fn list_visible(&self) -> Vec<WindowRef>;
    /// Ask the OS to bring `window` to the front. Returning `Ok` only means the
    /// request was accepted — the bridge always confirms with [`Self::foreground`].
    fn focus(&self, window: &WindowRef) -> Result<(), PlatformError>;
}

/// Injecting keystrokes into whatever window currently has focus.
pub trait InputPort: Send {
    fn send(&self, steps: &[SendStep]) -> Result<(), PlatformError>;
}

/// Time, injected so the focus-confirmation loop can be tested without waiting.
pub trait Clock: Send {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u64);
}

/// The full set of capabilities the bridge needs from its host.
pub struct Ports {
    pub clipboard: Box<dyn ClipboardPort>,
    pub windows: Box<dyn WindowPort>,
    pub input: Box<dyn InputPort>,
    pub clock: Box<dyn Clock>,
}
