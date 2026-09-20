//! End-to-end behaviour of the bridge, driven entirely through the fake ports.
//!
//! These cover the flow the user actually experiences: clicking a coordinate in
//! a guide, and every way that can fail.

use zaapy_core::bridge::{BridgeError, IgnoreReason, Outcome};
use zaapy_core::config::{Config, TargetSelector};
use zaapy_core::mock::{window, FocusBehaviour, MockRig};
use zaapy_core::platform::{Clock, Key, PlatformError, SendStep, WindowRef};
use zaapy_core::{Bridge, Verb};

const GANYMEDE: u64 = 1;
const DOFUS: u64 = 2;
const EDITOR: u64 = 3;

fn windows() -> Vec<WindowRef> {
    vec![
        window(GANYMEDE, "Ganymede.exe", "Ganymède"),
        window(DOFUS, "Dofus.exe", "Miniluchi - Dofus 3.0"),
        window(EDITOR, "notepad.exe", "notes.txt"),
    ]
}

fn config() -> Config {
    Config {
        source_processes: vec!["Ganymede.exe".to_string()],
        target: TargetSelector {
            process: "Dofus.exe".to_string(),
            title_pattern: String::new(),
        },
        ..Config::default()
    }
}

/// Build a rig whose first tick has already established the clipboard baseline.
fn armed(config: Config) -> (MockRig, Bridge) {
    let rig = MockRig::new(windows());
    let mut bridge = Bridge::new(rig.ports(), config);
    rig.windows.set_foreground(Some(GANYMEDE));
    assert!(bridge.tick().is_none(), "first tick only takes a baseline");
    (rig, bridge)
}

#[test]
fn a_click_in_ganymede_reaches_the_game() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/travel 1,2");
    let report = bridge
        .tick()
        .expect("the clipboard change should be handled");

    assert_eq!(
        report.outcome,
        Outcome::Sent {
            command: "/travel 1,2".to_string(),
            target_title: "Miniluchi - Dofus 3.0".to_string(),
            clipboard_cleared: true,
        }
    );
    assert_eq!(rig.windows.focus_calls(), vec![DOFUS]);
    assert_eq!(rig.input.sends().len(), 1);
    assert_eq!(rig.input.sends()[0], config().send_sequence);
}

/// The two waits that stand between a working bridge and one whose keystrokes
/// vanish: the game is not reading input the instant its window comes forward,
/// and it has not pasted the instant injection returns.
#[test]
fn the_game_is_given_time_to_open_its_chat_and_to_paste() {
    let (rig, mut bridge) = armed(Config {
        // A sequence with no delays of its own, so the clock measures nothing but
        // the two waits under test.
        send_sequence: vec![SendStep::key(Key::Enter)],
        focus_settle_ms: 300,
        clipboard_clear_delay_ms: 400,
        ..config()
    });

    rig.clipboard.copy("/travel 1,2");
    bridge
        .tick()
        .expect("the clipboard change should be handled");

    assert_eq!(rig.clock.now_ms(), 700);
    assert_eq!(rig.clipboard.clears(), 1);
}

#[test]
fn the_clipboard_is_emptied_only_after_a_successful_send() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/travel 1,2");
    bridge.tick();

    assert_eq!(rig.clipboard.clears(), 1);
    assert_eq!(rig.clipboard.text(), None);
}

/// Emptying the clipboard is the one part of a send that can quietly not happen,
/// so the outcome carries whether it did — the log is how the user finds out.
#[test]
fn a_send_reports_whether_the_clipboard_was_emptied() {
    let (rig, mut bridge) = armed(Config {
        clear_clipboard_on_success: false,
        ..config()
    });

    rig.clipboard.copy("/travel 1,2");
    let report = bridge
        .tick()
        .expect("the clipboard change should be handled");

    assert!(matches!(
        report.outcome,
        Outcome::Sent {
            clipboard_cleared: false,
            ..
        }
    ));
    assert_eq!(rig.clipboard.text().as_deref(), Some("/travel 1,2"));
}

#[test]
fn a_failed_send_leaves_the_command_available_for_a_manual_paste() {
    let (rig, mut bridge) = armed(config());
    rig.windows
        .set_windows(vec![window(GANYMEDE, "Ganymede.exe", "Ganymède")]);

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert!(matches!(
        report.outcome,
        Outcome::Failed {
            error: BridgeError::TargetNotFound,
            ..
        }
    ));
    assert_eq!(rig.clipboard.text().as_deref(), Some("/travel 1,2"));
    assert_eq!(rig.clipboard.clears(), 0);
}

/// The safety gate: copying a command from anywhere else must do nothing.
#[test]
fn a_command_copied_outside_ganymede_is_ignored() {
    let (rig, mut bridge) = armed(config());
    rig.windows.set_foreground(Some(EDITOR));
    bridge.tick(); // let the editor become both samples

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert_eq!(
        report.outcome,
        Outcome::Ignored {
            reason: IgnoreReason::SourceNotFocused {
                foreground: Some("notepad.exe".to_string()),
            }
        }
    );
    assert!(rig.input.sends().is_empty());
}

/// The clipboard can change at any point between two samples, so a command
/// copied just as the user leaves Ganymède still counts.
#[test]
fn the_previous_foreground_sample_still_opens_the_gate() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/travel 1,2");
    rig.windows.set_foreground(Some(EDITOR));
    let report = bridge.tick().unwrap();

    assert!(matches!(report.outcome, Outcome::Sent { .. }));
}

#[test]
fn ordinary_copied_text_is_ignored() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("rendez-vous devant le zaap");
    let report = bridge.tick().unwrap();

    assert!(matches!(
        report.outcome,
        Outcome::Ignored {
            reason: IgnoreReason::Unparsed { .. }
        }
    ));
    assert!(rig.input.sends().is_empty());
}

#[test]
fn zaap_is_recognised_but_not_relayed_until_it_ships() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/zaap Astrub");
    let report = bridge.tick().unwrap();

    assert_eq!(
        report.outcome,
        Outcome::Ignored {
            reason: IgnoreReason::CommandDisabled { verb: Verb::Zaap }
        }
    );
    assert!(rig.input.sends().is_empty());
}

#[test]
fn enabling_zaap_is_a_single_switch() {
    let mut config = config();
    config.commands.zaap = true;
    let (rig, mut bridge) = armed(config);

    rig.clipboard.copy("/zaap Astrub");
    assert!(matches!(
        bridge.tick().unwrap().outcome,
        Outcome::Sent { .. }
    ));
    assert_eq!(rig.input.sends().len(), 1);
}

/// Clipboard notifications like to fire twice for one copy.
#[test]
fn the_same_command_twice_in_a_row_travels_once() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/travel 1,2");
    bridge.tick();
    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert_eq!(
        report.outcome,
        Outcome::Ignored {
            reason: IgnoreReason::Duplicate
        }
    );
    assert_eq!(rig.input.sends().len(), 1);
}

#[test]
fn the_same_destination_can_be_revisited_after_the_dedup_window() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy("/travel 1,2");
    bridge.tick();
    rig.clock.advance(600);
    rig.clipboard.copy("/travel 1,2");

    assert!(matches!(
        bridge.tick().unwrap().outcome,
        Outcome::Sent { .. }
    ));
    assert_eq!(rig.input.sends().len(), 2);
}

#[test]
fn two_matching_clients_are_reported_as_ambiguous_rather_than_guessed() {
    let (rig, mut bridge) = armed(config());
    let mut open = windows();
    open.push(window(4, "Dofus.exe", "Autre - Dofus 3.0"));
    rig.windows.set_windows(open);

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert!(matches!(
        report.outcome,
        Outcome::Failed {
            error: BridgeError::TargetAmbiguous { count: 2 },
            ..
        }
    ));
    assert!(rig.input.sends().is_empty());
}

#[test]
fn a_title_filter_picks_one_client_out_of_several() {
    let mut config = config();
    config.target.title_pattern = "Miniluchi".to_string();
    let (rig, mut bridge) = armed(config);
    let mut open = windows();
    open.push(window(4, "Dofus.exe", "Autre - Dofus 3.0"));
    rig.windows.set_windows(open);

    rig.clipboard.copy("/travel 1,2");

    assert!(matches!(
        bridge.tick().unwrap().outcome,
        Outcome::Sent { .. }
    ));
    assert_eq!(rig.windows.focus_calls(), vec![DOFUS]);
}

/// Windows' foreground lock: the call succeeds, the window stays put. Nothing
/// may be typed in that situation.
#[test]
fn keystrokes_are_never_sent_to_a_window_that_did_not_come_forward() {
    let (rig, mut bridge) = armed(config());
    rig.windows.set_behaviour(FocusBehaviour::Ignores);

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert!(matches!(
        report.outcome,
        Outcome::Failed {
            error: BridgeError::FocusFailed { .. },
            ..
        }
    ));
    assert!(rig.input.sends().is_empty(), "nothing may be typed blind");
    assert_eq!(rig.clipboard.text().as_deref(), Some("/travel 1,2"));
}

#[test]
fn a_refused_focus_request_is_reported_with_its_cause() {
    let (rig, mut bridge) = armed(config());
    rig.windows
        .set_behaviour(FocusBehaviour::Rejects("access denied".to_string()));

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    match report.outcome {
        Outcome::Failed {
            error: BridgeError::FocusRejected { detail },
            ..
        } => assert!(detail.contains("access denied"), "{detail}"),
        other => panic!("unexpected outcome: {other:?}"),
    }
}

/// The elevated-Dofus case on Windows: focus works, keystrokes do not.
#[test]
fn refused_keystrokes_surface_as_a_failure_the_user_can_act_on() {
    let (rig, mut bridge) = armed(config());
    rig.input.fail_with(PlatformError::InputRejected {
        detail: "UIPI".to_string(),
    });

    rig.clipboard.copy("/travel 1,2");
    let report = bridge.tick().unwrap();

    assert!(matches!(
        report.outcome,
        Outcome::Failed {
            error: BridgeError::InputFailed { .. },
            ..
        }
    ));
    assert!(report.outcome.should_notify());
    assert_eq!(rig.clipboard.clears(), 0);
}

#[test]
fn whatever_sat_in_the_clipboard_at_startup_is_never_replayed() {
    let rig = MockRig::new(windows());
    rig.clipboard.copy("/travel 9,9");
    rig.windows.set_foreground(Some(GANYMEDE));
    let mut bridge = Bridge::new(rig.ports(), config());

    assert!(bridge.tick().is_none());
    assert!(bridge.tick().is_none());
    assert!(rig.input.sends().is_empty());
}

#[test]
fn the_master_switch_stops_the_bridge_without_replaying_on_resume() {
    let mut config = config();
    config.enabled = false;
    let (rig, mut bridge) = armed(config.clone());

    rig.clipboard.copy("/travel 1,2");
    assert!(bridge.tick().is_none());
    assert!(rig.input.sends().is_empty());

    config.enabled = true;
    bridge.set_config(config);
    assert!(
        bridge.tick().is_none(),
        "re-enabling must not fire on a command copied while off"
    );
}

#[test]
fn a_clipboard_holding_an_image_is_simply_skipped() {
    let (rig, mut bridge) = armed(config());

    rig.clipboard.copy_non_text();

    assert!(bridge.tick().is_none());
    assert!(rig.input.sends().is_empty());
}
