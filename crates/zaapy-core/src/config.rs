//! User configuration: what to listen to, what to drive, and how to type.
//!
//! Every field that depends on the user's actual machine — the Ganymède
//! executable name, the Dofus window title — is data rather than a constant.
//! Zaapy does not know what those are called, and the settings page fills them
//! in from the live window list.

use serde::{Deserialize, Serialize};

use crate::command::Verb;
use crate::platform::{Key, Modifier, SendStep, WindowRef};

pub const CONFIG_VERSION: u32 = 1;

/// Bounds on the focus confirmation wait. Below the floor the game has no time
/// to come forward; above the ceiling the user is just staring at a frozen guide.
const FOCUS_TIMEOUT_RANGE_MS: (u64, u64) = (100, 5_000);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    /// Master switch, flipped from the tray menu.
    pub enabled: bool,
    /// Executable names allowed to trigger the bridge. A command copied while
    /// anything else is in front is ignored — this is the safety gate the whole
    /// product rests on.
    pub source_processes: Vec<String>,
    pub target: TargetSelector,
    pub commands: EnabledCommands,
    pub send_sequence: Vec<SendStep>,
    pub focus_timeout_ms: u64,
    pub clear_clipboard_on_success: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            enabled: true,
            source_processes: Vec::new(),
            target: TargetSelector::default(),
            commands: EnabledCommands::default(),
            send_sequence: default_send_sequence(),
            focus_timeout_ms: 800,
            clear_clipboard_on_success: true,
        }
    }
}

impl Config {
    /// Clamp anything a hand-edited config file could get wrong, so the bridge
    /// never has to defend against its own settings.
    pub fn sanitised(mut self) -> Self {
        self.focus_timeout_ms = self
            .focus_timeout_ms
            .clamp(FOCUS_TIMEOUT_RANGE_MS.0, FOCUS_TIMEOUT_RANGE_MS.1);
        if self.send_sequence.is_empty() {
            self.send_sequence = default_send_sequence();
        }
        self.source_processes.retain(|name| !name.trim().is_empty());
        self
    }

    /// Whether `window` is an application allowed to trigger the bridge.
    pub fn is_source(&self, window: &WindowRef) -> bool {
        self.source_processes
            .iter()
            .any(|name| window.process_matches(name))
    }

    /// True once the user has told Zaapy both what to listen to and what to drive.
    pub fn is_configured(&self) -> bool {
        !self.source_processes.is_empty() && !self.target.process.trim().is_empty()
    }
}

/// How the target window is remembered between sessions.
///
/// Never by handle: an HWND does not survive a game restart. The pair is
/// re-resolved against the live window list on every single send.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetSelector {
    pub process: String,
    /// Case-insensitive substring of the window title, typically the character
    /// name. Empty means "any window of that process" — fine for a single
    /// client, ambiguous the moment a second one is running.
    pub title_pattern: String,
}

impl TargetSelector {
    pub fn matches(&self, window: &WindowRef) -> bool {
        if !window.process_matches(&self.process) {
            return false;
        }
        let pattern = self.title_pattern.trim();
        pattern.is_empty()
            || window
                .title
                .to_lowercase()
                .contains(&pattern.to_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnabledCommands {
    pub travel: bool,
    /// Off by default: the command does not exist in game yet, so relaying it
    /// would only type a rejected line into the chat.
    pub zaap: bool,
}

impl Default for EnabledCommands {
    fn default() -> Self {
        Self {
            travel: true,
            zaap: false,
        }
    }
}

impl EnabledCommands {
    pub fn allows(&self, verb: Verb) -> bool {
        match verb {
            Verb::Travel => self.travel,
            Verb::Zaap => self.zaap,
        }
    }
}

/// Open the chat, paste, send.
///
/// Pasting rather than typing is deliberate: the command is already in the
/// clipboard, and it sidesteps keyboard layouts entirely (an AZERTY user typing
/// a comma would otherwise need a different key than a QWERTY one).
pub fn default_send_sequence() -> Vec<SendStep> {
    vec![
        SendStep::key(Key::Enter),
        SendStep::delay(60),
        SendStep::chord(vec![Modifier::Primary], Key::Char('v')),
        SendStep::delay(60),
        SendStep::key(Key::Enter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(process: &str, title: &str) -> WindowRef {
        WindowRef {
            handle: 1,
            pid: 2,
            process: process.to_string(),
            title: title.to_string(),
        }
    }

    #[test]
    fn target_matching_ignores_case_on_both_process_and_title() {
        let selector = TargetSelector {
            process: "dofus.exe".to_string(),
            title_pattern: "miniluchi".to_string(),
        };
        assert!(selector.matches(&window("Dofus.exe", "Miniluchi - Dofus 3.0")));
        assert!(!selector.matches(&window("Dofus.exe", "Autre - Dofus 3.0")));
        assert!(!selector.matches(&window("Ganymede.exe", "Miniluchi")));
    }

    #[test]
    fn empty_title_pattern_matches_any_window_of_the_process() {
        let selector = TargetSelector {
            process: "Dofus.exe".to_string(),
            title_pattern: "  ".to_string(),
        };
        assert!(selector.matches(&window("Dofus.exe", "anything at all")));
    }

    #[test]
    fn sanitising_repairs_a_hand_edited_file() {
        let config = Config {
            focus_timeout_ms: 10,
            send_sequence: Vec::new(),
            source_processes: vec!["Ganymede.exe".into(), "  ".into()],
            ..Config::default()
        }
        .sanitised();

        assert_eq!(config.focus_timeout_ms, FOCUS_TIMEOUT_RANGE_MS.0);
        assert_eq!(config.send_sequence, default_send_sequence());
        assert_eq!(config.source_processes, vec!["Ganymede.exe".to_string()]);
    }

    #[test]
    fn zaap_stays_off_until_ankama_ships_it() {
        let config = Config::default();
        assert!(config.commands.allows(Verb::Travel));
        assert!(!config.commands.allows(Verb::Zaap));
    }

    #[test]
    fn a_partial_config_file_still_loads() {
        // Forward compatibility: a file written by an older build, or trimmed by
        // hand, must not stop the app from starting.
        let config: Config = serde_json::from_str(r#"{"source_processes":["Ganymede.exe"]}"#)
            .expect("partial config should deserialise");
        assert_eq!(config.send_sequence, default_send_sequence());
        assert!(config.enabled);
    }

    #[test]
    fn send_sequence_survives_a_json_round_trip() {
        let encoded = serde_json::to_string(&default_send_sequence()).unwrap();
        assert!(encoded.contains(r#""type":"key""#), "{encoded}");
        let decoded: Vec<SendStep> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, default_send_sequence());
    }
}
