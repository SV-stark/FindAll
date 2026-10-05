//! Global hotkey parsing and the subscription identity that depends on it.
//!
//! Extracted from `iced_ui::mod` because it is a pure function with no reference
//! to `App` or `Message`: it can be tested directly and its behaviour when
//! re-subscribing is easier to reason about in isolation.
//!
//! The parsing is hand-rolled rather than delegated to `global_hotkey`'s own
//! `FromStr`, because the user-facing format allows friendly aliases
//! (`win`, `super`, `command`, `control`) and is stored in `settings.json` as a
//! plain string.

use std::hash::{Hash, Hasher};

/// Subscription identity for the system-level streams.
///
/// Recreating a `Subscription` only when one of these changes is what makes Iced
/// drop the old one: `Subscription::run_with_id` compares identity, and rebuilding
/// a global hotkey registration on every frame would thrash the OS.
#[derive(Debug, Clone)]
pub(super) struct SystemSubscriptionData {
    pub hotkey_str: String,
    pub minimize_to_tray: bool,
}

impl Hash for SystemSubscriptionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hotkey_str.hash(state);
        self.minimize_to_tray.hash(state);
    }
}

impl PartialEq for SystemSubscriptionData {
    fn eq(&self, other: &Self) -> bool {
        self.hotkey_str == other.hotkey_str && self.minimize_to_tray == other.minimize_to_tray
    }
}

impl Eq for SystemSubscriptionData {}

/// Parses a `"Ctrl+Shift+K"`-style string into a hotkey.
///
/// Returns `None` when no key is present or the key is unrecognised. A trailing
/// bare modifier (`"Ctrl+"`) is rejected for the same reason: a hotkey with no
/// key would either register nothing or register the bare modifier alone.
pub(super) fn parse_hotkey(s: &str) -> Option<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    let parts: Vec<&str> = s
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "alt" => modifiers.insert(Modifiers::ALT),
            "ctrl" | "control" => modifiers.insert(Modifiers::CONTROL),
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "meta" | "win" | "super" | "command" => modifiers.insert(Modifiers::SUPER),
            "space" => key_code = Some(Code::Space),
            k => {
                if k.chars().count() == 1 {
                    key_code = match k.chars().next().unwrap().to_ascii_uppercase() {
                        'A' => Some(Code::KeyA),
                        'B' => Some(Code::KeyB),
                        'C' => Some(Code::KeyC),
                        'D' => Some(Code::KeyD),
                        'E' => Some(Code::KeyE),
                        'F' => Some(Code::KeyF),
                        'G' => Some(Code::KeyG),
                        'H' => Some(Code::KeyH),
                        'I' => Some(Code::KeyI),
                        'J' => Some(Code::KeyJ),
                        'K' => Some(Code::KeyK),
                        'L' => Some(Code::KeyL),
                        'M' => Some(Code::KeyM),
                        'N' => Some(Code::KeyN),
                        'O' => Some(Code::KeyO),
                        'P' => Some(Code::KeyP),
                        'Q' => Some(Code::KeyQ),
                        'R' => Some(Code::KeyR),
                        'S' => Some(Code::KeyS),
                        'T' => Some(Code::KeyT),
                        'U' => Some(Code::KeyU),
                        'V' => Some(Code::KeyV),
                        'W' => Some(Code::KeyW),
                        'X' => Some(Code::KeyX),
                        'Y' => Some(Code::KeyY),
                        'Z' => Some(Code::KeyZ),
                        '0' => Some(Code::Digit0),
                        '1' => Some(Code::Digit1),
                        '2' => Some(Code::Digit2),
                        '3' => Some(Code::Digit3),
                        '4' => Some(Code::Digit4),
                        '5' => Some(Code::Digit5),
                        '6' => Some(Code::Digit6),
                        '7' => Some(Code::Digit7),
                        '8' => Some(Code::Digit8),
                        '9' => Some(Code::Digit9),
                        _ => None,
                    };
                }
            }
        }
    }

    key_code.map(|code| HotKey::new(Some(modifiers), code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifier_plus_letter() {
        assert!(parse_hotkey("Ctrl+Shift+K").is_some());
    }

    #[test]
    fn accepts_friendly_aliases() {
        // These are the forms a user is likely to type and which appear in
        // settings.json.
        for input in ["win+space", "super+space", "meta+space", "command+space"] {
            assert!(parse_hotkey(input).is_some(), "failed: {input}");
        }
        for input in ["ctrl+a", "control+a"] {
            assert!(parse_hotkey(input).is_some(), "failed: {input}");
        }
    }

    #[test]
    fn accepts_letters_digits_and_space() {
        assert!(parse_hotkey("Alt+7").is_some());
        assert!(parse_hotkey("Alt+Space").is_some());
    }

    #[test]
    fn is_case_insensitive() {
        assert!(parse_hotkey("CTRL+SHIFT+K").is_some());
        assert!(parse_hotkey("ctrl+shift+k").is_some());
    }

    #[test]
    fn tolerates_whitespace_around_parts() {
        assert!(parse_hotkey(" Ctrl + Shift + K ").is_some());
    }

    #[test]
    fn rejects_modifier_only() {
        // A hotkey with no key would register nothing useful, or worse, register
        // the bare modifier.
        assert!(parse_hotkey("Ctrl").is_none());
        assert!(parse_hotkey("Ctrl+").is_none());
        assert!(parse_hotkey("Ctrl+Shift+").is_none());
    }

    #[test]
    fn rejects_unknown_and_empty_input() {
        assert!(parse_hotkey("").is_none());
        assert!(parse_hotkey("Ctrl+NotAKey").is_none());
        assert!(parse_hotkey("F13").is_none());
    }

    #[test]
    fn subscription_identity_tracks_both_fields() {
        let base = SystemSubscriptionData {
            hotkey_str: "Ctrl+K".to_string(),
            minimize_to_tray: true,
        };
        assert_eq!(base, base.clone());

        let other_hotkey = SystemSubscriptionData {
            hotkey_str: "Ctrl+J".to_string(),
            minimize_to_tray: true,
        };
        assert_ne!(base, other_hotkey);

        let other_tray = SystemSubscriptionData {
            hotkey_str: "Ctrl+K".to_string(),
            minimize_to_tray: false,
        };
        assert_ne!(base, other_tray);
    }
}
