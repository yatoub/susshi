//! Wallix menu parser and selection logic.
//!
//! This module provides utilities to parse Wallix's interactive menu output
//! and automatically select an entry based on target (user@account@host:protocol)
//! and group matching.

use crate::config::ResolvedServer;
use anyhow::{Result, anyhow};

#[path = "target.rs"]
mod target;
pub(crate) use target::group_suffix_matches;
pub use target::{
    WallixMenuEntry, build_expected_groups, build_expected_target, build_expected_targets,
};

#[path = "menu.rs"]
mod menu;
pub use menu::parse_wallix_menu;

#[path = "select.rs"]
mod select;
pub use select::{select_id_by_target_and_group, select_id_for_server};

/// Strip ANSI/VT100 escape sequences from a string.
pub(crate) fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    // consume until a letter (the final byte of a CSI sequence)
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                Some('(') | Some(')') | Some('*') | Some('+') => {
                    chars.next();
                    chars.next(); // consume one more designator byte
                }
                _ => {
                    chars.next(); // consume whatever follows ESC
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
