//! In-memory stand-ins for the platform ports.
//!
//! These let the full bridge flow — including every failure path — run on a
//! machine with neither Dofus nor Ganymède, which matters here: Windows is the
//! primary target and most development happens elsewhere.

use std::sync::{Arc, Mutex};

use crate::platform::{
    ClipboardPort, Clock, InputPort, PlatformError, Ports, SendStep, WindowPort, WindowRef,
};

/// Build a window reference the way the real platform layer would.
pub fn window(handle: u64, process: &str, title: &str) -> WindowRef {
    WindowRef {
        handle,
        pid: handle as u32 + 1000,
        process: process.to_string(),
        title: title.to_string(),
    }
}

#[derive(Debug, Default)]
struct ClipboardState {
    sequence: u64,
    text: Option<String>,
    clears: usize,
}

/// A clipboard whose content the test drives directly.
#[derive(Debug, Clone, Default)]
pub struct MockClipboard {
    state: Arc<Mutex<ClipboardState>>,
}

impl MockClipboard {
    /// Simulate the user (or Ganymède) copying something.
    pub fn copy(&self, text: &str) {
        let mut state = self.state.lock().unwrap();
        state.sequence += 1;
        state.text = Some(text.to_string());
    }

    /// Simulate a clipboard holding something that is not text at all.
    pub fn copy_non_text(&self) {
        let mut state = self.state.lock().unwrap();
        state.sequence += 1;
        state.text = None;
    }

    pub fn text(&self) -> Option<String> {
        self.state.lock().unwrap().text.clone()
    }

    /// How many times the bridge asked for the clipboard to be emptied.
    pub fn clears(&self) -> usize {
        self.state.lock().unwrap().clears
    }
}

impl ClipboardPort for MockClipboard {
    fn sequence(&self) -> u64 {
        self.state.lock().unwrap().sequence
    }

    fn read_text(&self) -> Option<String> {
        self.state.lock().unwrap().text.clone()
    }

    fn clear_if_matches(&self, expected: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.text.as_deref() != Some(expected) {
            return false;
        }
        state.clears += 1;
        state.sequence += 1;
        state.text = None;
        true
    }
}

/// How the fake window manager reacts to a focus request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusBehaviour {
    /// The window comes forward, as it does on a cooperative system.
    Grants,
    /// The call succeeds but nothing moves — Windows' foreground lock, which is
    /// exactly why the bridge confirms instead of trusting the return value.
    Ignores,
    Rejects(String),
}

#[derive(Debug)]
struct WindowState {
    windows: Vec<WindowRef>,
    foreground: Option<u64>,
    behaviour: FocusBehaviour,
    focus_calls: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct MockWindows {
    state: Arc<Mutex<WindowState>>,
}

impl MockWindows {
    pub fn new(windows: Vec<WindowRef>) -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowState {
                windows,
                foreground: None,
                behaviour: FocusBehaviour::Grants,
                focus_calls: Vec::new(),
            })),
        }
    }

    pub fn set_foreground(&self, handle: Option<u64>) {
        self.state.lock().unwrap().foreground = handle;
    }

    pub fn set_behaviour(&self, behaviour: FocusBehaviour) {
        self.state.lock().unwrap().behaviour = behaviour;
    }

    pub fn set_windows(&self, windows: Vec<WindowRef>) {
        self.state.lock().unwrap().windows = windows;
    }

    pub fn focus_calls(&self) -> Vec<u64> {
        self.state.lock().unwrap().focus_calls.clone()
    }
}

impl WindowPort for MockWindows {
    fn foreground(&self) -> Option<WindowRef> {
        let state = self.state.lock().unwrap();
        let handle = state.foreground?;
        state.windows.iter().find(|w| w.handle == handle).cloned()
    }

    fn list_visible(&self) -> Vec<WindowRef> {
        self.state.lock().unwrap().windows.clone()
    }

    fn focus(&self, window: &WindowRef) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();
        state.focus_calls.push(window.handle);
        match state.behaviour.clone() {
            FocusBehaviour::Grants => {
                state.foreground = Some(window.handle);
                Ok(())
            }
            FocusBehaviour::Ignores => Ok(()),
            FocusBehaviour::Rejects(detail) => Err(PlatformError::FocusRejected { detail }),
        }
    }
}

#[derive(Debug, Default)]
struct InputState {
    sends: Vec<Vec<SendStep>>,
    failure: Option<PlatformError>,
}

#[derive(Debug, Clone, Default)]
pub struct MockInput {
    state: Arc<Mutex<InputState>>,
}

impl MockInput {
    /// Make every subsequent send fail — the elevated-target case on Windows.
    pub fn fail_with(&self, error: PlatformError) {
        self.state.lock().unwrap().failure = Some(error);
    }

    pub fn sends(&self) -> Vec<Vec<SendStep>> {
        self.state.lock().unwrap().sends.clone()
    }
}

impl InputPort for MockInput {
    fn send(&self, steps: &[SendStep]) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();
        if let Some(failure) = state.failure.clone() {
            return Err(failure);
        }
        state.sends.push(steps.to_vec());
        Ok(())
    }
}

/// Virtual time: sleeping advances the clock instead of blocking, so the focus
/// timeout can be exercised instantly.
#[derive(Debug, Clone, Default)]
pub struct MockClock {
    now: Arc<Mutex<u64>>,
}

impl MockClock {
    pub fn advance(&self, ms: u64) {
        *self.now.lock().unwrap() += ms;
    }
}

impl Clock for MockClock {
    fn now_ms(&self) -> u64 {
        *self.now.lock().unwrap()
    }

    fn sleep_ms(&self, ms: u64) {
        self.advance(ms);
    }
}

/// A complete set of fake ports, plus handles onto each of them.
pub struct MockRig {
    pub clipboard: MockClipboard,
    pub windows: MockWindows,
    pub input: MockInput,
    pub clock: MockClock,
}

impl MockRig {
    pub fn new(windows: Vec<WindowRef>) -> Self {
        Self {
            clipboard: MockClipboard::default(),
            windows: MockWindows::new(windows),
            input: MockInput::default(),
            clock: MockClock::default(),
        }
    }

    pub fn ports(&self) -> Ports {
        Ports {
            clipboard: Box::new(self.clipboard.clone()),
            windows: Box::new(self.windows.clone()),
            input: Box::new(self.input.clone()),
            clock: Box::new(self.clock.clone()),
        }
    }
}
