//! The platform-independent core of Zaapy.
//!
//! Zaapy bridges two windows: Ganymède copies a `/travel x,y` command to the
//! clipboard when the user clicks a coordinate in a guide, and Zaapy delivers it
//! as keystrokes to a Dofus client. This crate owns everything about that flow
//! except the operating system calls themselves, which sit behind the traits in
//! [`platform`] and are implemented by the Tauri application.
//!
//! See `docs/VISION.md` for the product and `docs/ARCHITECTURE.md` for the
//! system design.

pub mod bridge;
pub mod command;
pub mod config;
pub mod mock;
pub mod platform;

pub use bridge::{Bridge, BridgeError, IgnoreReason, Outcome, TickReport, TICK_INTERVAL_MS};
pub use command::{GameCommand, Rejection, Verb};
pub use config::Config;
pub use platform::{
    ClipboardPort, Clock, InputPort, Key, Modifier, PlatformError, Ports, SendStep, WindowPort,
    WindowRef,
};
