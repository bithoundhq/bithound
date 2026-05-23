//! Shared parser for the ten dotted-namespace name newtypes.
//!
//! Every name newtype (`IncidentKind`, `MetricName`, `SignalName`,
//! `StateName`, `HealthTargetId`, `CapabilityName`, `EventName`,
//! `TransitionName`, `InventoryName`, `DiagnosisName`) is constructed
//! through [`parse_dotted_name`] so the inner string is guaranteed to
//! satisfy a single shared grammar.
//!
//! # Grammar
//!
//! ```text
//! name    = segment ("." segment)+
//! segment = [a-z] [a-z0-9_]*
//! ```
//!
//! - Two or more dot-separated segments.
//! - Each segment must start with an ASCII lowercase letter; subsequent
//!   characters may be lowercase letters, digits, or underscores.
//! - Total length is 1–128 bytes (the grammar is ASCII so bytes and
//!   characters coincide).
//!
//! ## Examples
//!
//! Valid: `bitcoin.tip_lag`, `lnd.channel.inactive`,
//! `host.disk.exhaustion`, `sidecar.collector.run_started`.
//!
//! Invalid: `tip_lag` (no dot), `BitcoinTipLag` (uppercase),
//! `bitcoin..tip_lag` (empty segment), `1bitcoin.x` (digit start),
//! `bitcoin.tip-lag` (hyphen), `""` (empty).
//!
//! # Why a hand-written parser
//!
//! The grammar is small enough that a one-pass byte scan beats pulling
//! in `regex`, and the error variants below carry positional
//! information the regex engine would not produce.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of bytes in a parsed name (equals number of
/// characters because the grammar is ASCII).
pub const MAX_NAME_LEN: usize = 128;

/// Validate `s` against the shared dotted-namespace grammar and return
/// it as an owned `String` on success.
///
/// The function takes a `&str` and returns a fresh `String` so callers
/// that already own a `String` can move it via `TryFrom<String>`
/// without an extra clone (the smart-constructor template wraps this
/// call and discards the input).
pub fn parse_dotted_name(s: &str) -> Result<String, ParseDottedNameError> {
    if s.is_empty() {
        return Err(ParseDottedNameError::Empty);
    }
    if s.len() > MAX_NAME_LEN {
        return Err(ParseDottedNameError::TooLong { got: s.len() });
    }

    let bytes = s.as_bytes();
    let mut saw_dot = false;
    let mut segment_start: usize = 0;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.' {
            saw_dot = true;
            if i == segment_start {
                return Err(ParseDottedNameError::EmptySegment { at: i });
            }
            segment_start = i + 1;
            continue;
        }

        if i == segment_start {
            if !b.is_ascii_lowercase() {
                if b == b'.' {
                    return Err(ParseDottedNameError::EmptySegment { at: i });
                }
                if is_segment_body_byte(b) {
                    return Err(ParseDottedNameError::BadSegmentStart { at: i });
                }
                return Err(ParseDottedNameError::BadCharacter {
                    at: i,
                    found: b as char,
                });
            }
        } else if !is_segment_body_byte(b) {
            return Err(ParseDottedNameError::BadCharacter {
                at: i,
                found: b as char,
            });
        }
    }

    if segment_start == bytes.len() {
        return Err(ParseDottedNameError::EmptySegment {
            at: bytes.len() - 1,
        });
    }

    if !saw_dot {
        return Err(ParseDottedNameError::NoDot);
    }

    Ok(s.to_string())
}

fn is_segment_body_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
}

/// Errors returned by [`parse_dotted_name`].
///
/// Positional variants (`BadCharacter`, `EmptySegment`,
/// `BadSegmentStart`) carry a byte offset `at` into the input string.
/// Because the grammar is ASCII the byte offset is also the character
/// offset.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParseDottedNameError {
    #[error("name is empty")]
    Empty,
    #[error("name exceeds {MAX_NAME_LEN} characters (got {got})")]
    TooLong { got: usize },
    #[error("invalid character {found:?} at position {at}")]
    BadCharacter { at: usize, found: char },
    #[error("empty segment at position {at}")]
    EmptySegment { at: usize },
    #[error("segment at position {at} must start with a-z")]
    BadSegmentStart { at: usize },
    #[error("name must contain at least one dot")]
    NoDot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_two_segment_name() {
        let parsed = parse_dotted_name("bitcoin.tip_lag").expect("valid");
        assert_eq!(parsed, "bitcoin.tip_lag");
    }

    #[test]
    fn accepts_three_segment_name() {
        let parsed = parse_dotted_name("lnd.channel.inactive").expect("valid");
        assert_eq!(parsed, "lnd.channel.inactive");
    }

    #[test]
    fn accepts_host_disk_exhaustion() {
        assert!(parse_dotted_name("host.disk.exhaustion").is_ok());
    }

    #[test]
    fn accepts_four_segment_name() {
        assert!(parse_dotted_name("sidecar.collector.run_started").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(parse_dotted_name(""), Err(ParseDottedNameError::Empty));
    }

    #[test]
    fn rejects_no_dot() {
        assert_eq!(
            parse_dotted_name("tip_lag"),
            Err(ParseDottedNameError::NoDot)
        );
    }

    #[test]
    fn rejects_uppercase_via_bad_character() {
        // The first uppercase byte fails at position 0 because the
        // segment start permits only `[a-z]`.
        assert_eq!(
            parse_dotted_name("BitcoinTipLag"),
            Err(ParseDottedNameError::BadCharacter { at: 0, found: 'B' })
        );
    }

    #[test]
    fn rejects_empty_segment_between_dots() {
        assert_eq!(
            parse_dotted_name("bitcoin..tip_lag"),
            Err(ParseDottedNameError::EmptySegment { at: 8 })
        );
    }

    #[test]
    fn rejects_segment_starting_with_digit() {
        assert_eq!(
            parse_dotted_name("1bitcoin.x"),
            Err(ParseDottedNameError::BadSegmentStart { at: 0 })
        );
    }

    #[test]
    fn rejects_hyphen_in_segment_body() {
        assert_eq!(
            parse_dotted_name("bitcoin.tip-lag"),
            Err(ParseDottedNameError::BadCharacter { at: 11, found: '-' })
        );
    }

    #[test]
    fn rejects_too_long() {
        let s = "a.".to_string() + &"a".repeat(MAX_NAME_LEN);
        let got = s.len();
        assert_eq!(
            parse_dotted_name(&s),
            Err(ParseDottedNameError::TooLong { got })
        );
    }

    #[test]
    fn accepts_max_length_boundary() {
        // Build a 128-byte name with one dot inside the body.
        let head = "a.";
        let tail = "a".repeat(MAX_NAME_LEN - head.len());
        let s = format!("{head}{tail}");
        assert_eq!(s.len(), MAX_NAME_LEN);
        assert!(parse_dotted_name(&s).is_ok());
    }

    #[test]
    fn rejects_trailing_dot_as_empty_segment() {
        assert_eq!(
            parse_dotted_name("bitcoin.tip_lag."),
            Err(ParseDottedNameError::EmptySegment { at: 15 })
        );
    }

    #[test]
    fn rejects_leading_dot_as_empty_segment() {
        assert_eq!(
            parse_dotted_name(".bitcoin"),
            Err(ParseDottedNameError::EmptySegment { at: 0 })
        );
    }

    #[test]
    fn underscore_at_segment_start_is_bad_segment_start() {
        assert_eq!(
            parse_dotted_name("_bitcoin.x"),
            Err(ParseDottedNameError::BadSegmentStart { at: 0 })
        );
    }
}
