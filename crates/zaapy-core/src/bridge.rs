//! The bridge itself: clipboard event in, keystrokes out.
//!
//! [`Bridge::tick`] holds the entire behaviour of the product and makes no
//! system call of its own — it only talks to [`crate::platform`] traits. That is
//! what lets the whole flow, including the failure paths, be tested on a machine
//! that has neither Dofus nor Ganymède installed.

use serde::{Deserialize, Serialize};

use crate::command::{self, GameCommand, Rejection, Verb};
use crate::config::{Config, TargetSelector};
use crate::platform::{PlatformError, Ports, WindowRef};

/// How often the host is expected to call [`Bridge::tick`].
pub const TICK_INTERVAL_MS: u64 = 100;

/// Clipboard notifications fire twice more often than one would like. A command
/// repeated inside this window is the same click, not a second one.
const DEDUP_WINDOW_MS: u64 = 500;

/// How tightly we re-check that the target actually came forward.
const FOCUS_POLL_MS: u64 = 25;

/// What the bridge did with one clipboard change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Sent {
        command: String,
        target_title: String,
    },
    Failed {
        command: String,
        error: BridgeError,
    },
    Ignored {
        reason: IgnoreReason,
    },
}

impl Outcome {
    /// Whether this deserves interrupting the user with a system notification.
    /// Only genuine failures qualify — a successful bridge should be silent.
    pub fn should_notify(&self) -> bool {
        matches!(self, Outcome::Failed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IgnoreReason {
    /// Not a command Zaapy relays. The common case, by far.
    Unparsed {
        reason: Rejection,
    },
    /// A real command, but copied while something other than the source app was
    /// in front. This is the gate that keeps Zaapy from reacting to the user's
    /// own copy-pasting.
    SourceNotFocused {
        foreground: Option<String>,
    },
    /// A known command the user has switched off — `/zaap` until it ships.
    CommandDisabled {
        verb: Verb,
    },
    Duplicate,
    /// No source app or no target picked yet.
    NotConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeError {
    #[error("no window matching the selected Dofus client is open")]
    TargetNotFound,
    #[error("{count} windows match the selected client; narrow the title filter")]
    TargetAmbiguous { count: usize },
    #[error("the system refused to raise the Dofus window: {detail}")]
    FocusRejected { detail: String },
    #[error("the Dofus window did not come to the foreground within {waited_ms} ms")]
    FocusFailed { waited_ms: u64 },
    #[error("keystrokes were refused by the system: {detail}")]
    InputFailed { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickReport {
    pub at_ms: u64,
    pub outcome: Outcome,
}

pub struct Bridge {
    ports: Ports,
    config: Config,
    /// `None` until the first tick, which only establishes a baseline. Without
    /// it, whatever happened to be in the clipboard at startup would fire.
    last_sequence: Option<u64>,
    previous_foreground: Option<WindowRef>,
    last_attempt: Option<(String, u64)>,
}

impl Bridge {
    pub fn new(ports: Ports, config: Config) -> Self {
        Self {
            ports,
            config: config.sanitised(),
            last_sequence: None,
            previous_foreground: None,
            last_attempt: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config.sanitised();
    }

    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Resolve the configured target against the windows open right now.
    ///
    /// Public because the settings page shows the result live, and the "test"
    /// button drives the same resolution.
    pub fn resolve_target(&self) -> Result<WindowRef, BridgeError> {
        resolve_target(&self.config.target, self.ports.windows.list_visible())
    }

    /// One turn of the loop. Returns `None` when nothing happened at all, which
    /// is the overwhelming majority of ticks.
    pub fn tick(&mut self) -> Option<TickReport> {
        let foreground = self.ports.windows.foreground();
        let previous = std::mem::replace(&mut self.previous_foreground, foreground.clone());

        let sequence = self.ports.clipboard.sequence();
        let changed = self.last_sequence.is_some_and(|last| last != sequence);
        self.last_sequence = Some(sequence);
        if !changed || !self.config.enabled {
            return None;
        }

        // Only now do we touch the content: reading the clipboard locks a shared
        // system resource, so we do it once per actual change and never on idle.
        let raw = self.ports.clipboard.read_text()?;
        let command = match command::parse(&raw) {
            Ok(command) => command,
            Err(reason) => {
                return self.report(Outcome::Ignored {
                    reason: IgnoreReason::Unparsed { reason },
                })
            }
        };

        if !self.config.commands.allows(command.verb()) {
            return self.report(Outcome::Ignored {
                reason: IgnoreReason::CommandDisabled {
                    verb: command.verb(),
                },
            });
        }

        // The safety gate. We accept the current foreground *or* the one sampled
        // a tick ago, because the clipboard may have changed at any point during
        // the interval between the two samples.
        let from_source = [foreground.as_ref(), previous.as_ref()]
            .into_iter()
            .flatten()
            .any(|window| self.config.is_source(window));
        if !from_source {
            return self.report(Outcome::Ignored {
                reason: IgnoreReason::SourceNotFocused {
                    foreground: foreground.map(|window| window.process),
                },
            });
        }

        if !self.config.is_configured() {
            return self.report(Outcome::Ignored {
                reason: IgnoreReason::NotConfigured,
            });
        }

        let now = self.ports.clock.now_ms();
        if let Some((previous_command, at)) = &self.last_attempt {
            if previous_command == &raw && now.saturating_sub(*at) < DEDUP_WINDOW_MS {
                return self.report(Outcome::Ignored {
                    reason: IgnoreReason::Duplicate,
                });
            }
        }
        self.last_attempt = Some((raw.clone(), now));

        let outcome = self.deliver(&command);
        self.report(outcome)
    }

    fn deliver(&self, command: &GameCommand) -> Outcome {
        let label = command.canonical();

        let target = match self.resolve_target() {
            Ok(target) => target,
            Err(error) => {
                return Outcome::Failed {
                    command: label,
                    error,
                }
            }
        };

        if let Err(error) = self.focus_and_confirm(&target) {
            return Outcome::Failed {
                command: label,
                error,
            };
        }

        // Confirming the window is in front is not confirming it is ready to read
        // input: a game just pulled out of the background drops the first frames'
        // worth of keystrokes, and the chat never opens.
        self.ports.clock.sleep_ms(self.config.focus_settle_ms);

        if let Err(error) = self.ports.input.send(&self.config.send_sequence) {
            return Outcome::Failed {
                command: label,
                error: input_failed(error),
            };
        }

        Outcome::Sent {
            command: label,
            target_title: target.title,
        }
    }

    /// Raise the window, then *verify* it actually came forward.
    ///
    /// The verification is what stands in for reading the screen: a keystroke
    /// sent to the wrong window is the one failure mode that could do real
    /// damage in game, and confirming the foreground rules it out.
    fn focus_and_confirm(&self, target: &WindowRef) -> Result<(), BridgeError> {
        self.ports
            .windows
            .focus(target)
            .map_err(|error| BridgeError::FocusRejected {
                detail: error.to_string(),
            })?;

        let started = self.ports.clock.now_ms();
        loop {
            if let Some(foreground) = self.ports.windows.foreground() {
                if foreground.handle == target.handle {
                    return Ok(());
                }
            }
            let waited = self.ports.clock.now_ms().saturating_sub(started);
            if waited >= self.config.focus_timeout_ms {
                return Err(BridgeError::FocusFailed { waited_ms: waited });
            }
            self.ports.clock.sleep_ms(FOCUS_POLL_MS);
        }
    }

    fn report(&self, outcome: Outcome) -> Option<TickReport> {
        Some(TickReport {
            at_ms: self.ports.clock.now_ms(),
            outcome,
        })
    }
}

/// Pick the one window a selector designates, refusing to guess between several.
///
/// Free-standing because the settings page resolves the target on every refresh
/// to show whether the bridge currently has somewhere to send to.
pub fn resolve_target(
    selector: &TargetSelector,
    windows: Vec<WindowRef>,
) -> Result<WindowRef, BridgeError> {
    let mut matches = windows
        .into_iter()
        .filter(|window| selector.matches(window));
    let first = matches.next().ok_or(BridgeError::TargetNotFound)?;
    let extra = matches.count();
    if extra > 0 {
        return Err(BridgeError::TargetAmbiguous { count: extra + 1 });
    }
    Ok(first)
}

fn input_failed(error: PlatformError) -> BridgeError {
    BridgeError::InputFailed {
        detail: error.to_string(),
    }
}
