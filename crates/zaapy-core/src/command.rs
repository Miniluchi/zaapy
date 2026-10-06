//! Parsing and validation of clipboard payloads.
//!
//! The clipboard is an untrusted channel: everything the user copies during a
//! session passes through here, and whatever survives validation gets typed
//! into a game window. So this module is deliberately closed and strict — it
//! recognises a short list of commands and rejects everything else, rather than
//! trying to sanitise arbitrary text. Plain text is the one exception: it is an
//! item name, and it is only ever pasted — never followed by `Enter` — so the
//! single-line and length checks are all it needs.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Longest payload we will ever relay. A `/travel` command is a fraction of
/// this; the cap exists so a pathological clipboard can never reach the chat.
pub const MAX_COMMAND_LEN: usize = 64;

/// Coordinates outside this range are not a Dofus map — most likely the user
/// copied something that merely looks like a command.
const COORDINATE_BOUND: i32 = 1_000;

/// The commands Zaapy knows how to relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Travel,
    Zaap,
    /// Plain text — an item name Ganymède copies on click. Pasted where the
    /// game's focus already is, without opening the chat.
    Item,
}

impl fmt::Display for Verb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verb::Travel => f.write_str("travel"),
            Verb::Zaap => f.write_str("zaap"),
            Verb::Item => f.write_str("item"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameCommand {
    Travel {
        x: i32,
        y: i32,
    },
    /// Announced by Ankama but not yet live in game. Parsed so that enabling it
    /// later is a configuration switch rather than a redesign; the argument
    /// grammar is intentionally loose until the real one exists.
    Zaap {
        destination: String,
    },
    Item {
        name: String,
    },
}

impl GameCommand {
    pub fn verb(&self) -> Verb {
        match self {
            GameCommand::Travel { .. } => Verb::Travel,
            GameCommand::Zaap { .. } => Verb::Zaap,
            GameCommand::Item { .. } => Verb::Item,
        }
    }

    /// The command as Zaapy would write it itself.
    ///
    /// Note that the normal path pastes the clipboard rather than typing this —
    /// it is used for the settings "test" button and for log lines.
    pub fn canonical(&self) -> String {
        match self {
            GameCommand::Travel { x, y } => format!("/travel {x},{y}"),
            GameCommand::Zaap { destination } => format!("/zaap {destination}"),
            GameCommand::Item { name } => name.clone(),
        }
    }
}

/// Why a clipboard payload was not accepted.
///
/// Rejections are the normal case — the user copies text all day long — so they
/// are never surfaced as notifications, only as debug traces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Rejection {
    #[error("clipboard is empty")]
    Empty,
    #[error("payload exceeds {MAX_COMMAND_LEN} characters")]
    TooLong,
    #[error("payload contains control characters")]
    ControlCharacters,
    #[error("payload is neither a command nor an item name")]
    NotACommand,
    #[error("`/{verb}` is not a command Zaapy relays")]
    UnknownVerb { verb: String },
    #[error("`/{verb}` arguments are malformed")]
    BadArguments { verb: Verb },
}

/// Validate a raw clipboard string and turn it into a command.
pub fn parse(raw: &str) -> Result<GameCommand, Rejection> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(Rejection::Empty);
    }
    if text.len() > MAX_COMMAND_LEN {
        return Err(Rejection::TooLong);
    }
    // Rejected before anything else: a newline in the payload would submit the
    // chat line early and let the rest of the clipboard type itself into the game.
    if text.chars().any(char::is_control) {
        return Err(Rejection::ControlCharacters);
    }

    let Some(body) = text.strip_prefix('/') else {
        return parse_item(text);
    };
    let (verb, args) = match body.split_once(char::is_whitespace) {
        Some((verb, args)) => (verb, args.trim()),
        None => (body, ""),
    };

    match verb.to_ascii_lowercase().as_str() {
        "travel" => parse_travel(args),
        "zaap" => parse_zaap(args),
        other => Err(Rejection::UnknownVerb {
            verb: other.to_string(),
        }),
    }
}

fn parse_travel(args: &str) -> Result<GameCommand, Rejection> {
    let bad = || Rejection::BadArguments { verb: Verb::Travel };
    // Guides write `1,2`; the treasure-hunt overlay writes `1 2`.
    let (x, y) = match args.split_once(',') {
        Some((x, y)) => (x.trim(), y.trim()),
        None => args.split_once(char::is_whitespace).ok_or_else(bad)?,
    };
    let x: i32 = x.trim().parse().map_err(|_| bad())?;
    let y: i32 = y.trim().parse().map_err(|_| bad())?;
    if x.abs() > COORDINATE_BOUND || y.abs() > COORDINATE_BOUND {
        return Err(bad());
    }
    Ok(GameCommand::Travel { x, y })
}

fn parse_zaap(args: &str) -> Result<GameCommand, Rejection> {
    let bad = || Rejection::BadArguments { verb: Verb::Zaap };
    if args.is_empty() {
        return Err(bad());
    }
    // Until the real grammar ships, accept what a place name plausibly contains
    // and nothing that could be read as punctuation by the chat parser.
    let acceptable = args
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '\'' | ','));
    if !acceptable {
        return Err(bad());
    }
    Ok(GameCommand::Zaap {
        destination: args.to_string(),
    })
}

/// Text with no letter in it — a number, a stray symbol — is not an item name.
fn parse_item(text: &str) -> Result<GameCommand, Rejection> {
    if !text.chars().any(char::is_alphabetic) {
        return Err(Rejection::NotACommand);
    }
    Ok(GameCommand::Item {
        name: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_ganymede_travel_command() {
        assert_eq!(parse("/travel 1,2"), Ok(GameCommand::Travel { x: 1, y: 2 }));
    }

    #[test]
    fn accepts_surrounding_whitespace_and_negative_coordinates() {
        assert_eq!(
            parse("  /travel -16,-42  "),
            Ok(GameCommand::Travel { x: -16, y: -42 })
        );
    }

    #[test]
    fn accepts_spaces_around_the_comma() {
        assert_eq!(
            parse("/travel 3 , 4"),
            Ok(GameCommand::Travel { x: 3, y: 4 })
        );
    }

    #[test]
    fn accepts_the_treasure_hunt_travel_command() {
        assert_eq!(
            parse("/travel -33 -59"),
            Ok(GameCommand::Travel { x: -33, y: -59 })
        );
    }

    #[test]
    fn rejects_empty_text() {
        assert_eq!(parse(""), Err(Rejection::Empty));
        assert_eq!(parse("   "), Err(Rejection::Empty));
    }

    #[test]
    fn plain_text_is_an_item_name() {
        assert_eq!(
            parse("  Épée de Boisaille "),
            Ok(GameCommand::Item {
                name: "Épée de Boisaille".to_string()
            })
        );
    }

    #[test]
    fn text_without_a_letter_is_not_an_item() {
        assert_eq!(parse("1234"), Err(Rejection::NotACommand));
        assert_eq!(parse("-,-"), Err(Rejection::NotACommand));
    }

    #[test]
    fn an_item_name_cannot_smuggle_a_second_line() {
        assert_eq!(parse("Bottes\n/quit"), Err(Rejection::ControlCharacters));
    }

    #[test]
    fn rejects_commands_zaapy_does_not_relay() {
        assert_eq!(
            parse("/who"),
            Err(Rejection::UnknownVerb {
                verb: "who".to_string()
            })
        );
    }

    #[test]
    fn rejects_malformed_coordinates() {
        for payload in [
            "/travel",
            "/travel 1",
            "/travel a,b",
            "/travel 1,2,3",
            "/travel 1 2 3",
        ] {
            assert_eq!(
                parse(payload),
                Err(Rejection::BadArguments { verb: Verb::Travel }),
                "payload: {payload}"
            );
        }
    }

    #[test]
    fn rejects_coordinates_no_map_could_have() {
        assert!(parse("/travel 99999,0").is_err());
    }

    /// The injection case: a payload that starts like a command but smuggles a
    /// second chat line behind a newline.
    #[test]
    fn rejects_embedded_newlines() {
        assert_eq!(
            parse("/travel 1,2\n/quit"),
            Err(Rejection::ControlCharacters)
        );
    }

    #[test]
    fn rejects_oversized_payloads() {
        let payload = format!("/travel 1,2{}", " ".repeat(MAX_COMMAND_LEN));
        // Trailing whitespace is trimmed, so pad with content instead.
        assert!(parse(&payload).is_ok());
        let payload = format!("/travel {}", "9".repeat(MAX_COMMAND_LEN));
        assert_eq!(parse(&payload), Err(Rejection::TooLong));
    }

    #[test]
    fn verb_matching_is_case_insensitive() {
        assert_eq!(parse("/TRAVEL 1,2"), Ok(GameCommand::Travel { x: 1, y: 2 }));
    }

    #[test]
    fn parses_zaap_ahead_of_its_release() {
        assert_eq!(
            parse("/zaap Astrub"),
            Ok(GameCommand::Zaap {
                destination: "Astrub".to_string()
            })
        );
        assert_eq!(
            parse("/zaap"),
            Err(Rejection::BadArguments { verb: Verb::Zaap })
        );
    }

    #[test]
    fn canonical_form_round_trips() {
        let command = parse("/travel 5 , 6").unwrap();
        assert_eq!(command.canonical(), "/travel 5,6");
        assert_eq!(parse(&command.canonical()), Ok(command));
    }
}
