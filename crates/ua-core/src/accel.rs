//! Keyboard accelerator parsing — the settings' "change shortcut" field.
//!
//! Accepts GTK-style strings like `<Super>v`, `<Ctrl><Shift>t`, `<Primary>p`
//! and canonicalizes them. Bare printable keys are rejected (they would
//! swallow typing globally); function/named keys are allowed with or
//! without modifiers.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accel {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_key: bool,
    /// Canonical key name: single char lowercased, or a named key (F1, Delete…).
    pub key: String,
}

const NAMED_KEYS: &[&str] = &[
    "Escape", "Delete", "Tab", "Insert", "Home", "End", "PageUp", "PageDown", "Space", "Enter",
    "Up", "Down", "Left", "Right",
];

fn is_named_key(k: &str) -> bool {
    NAMED_KEYS.contains(&k)
        || (k.len() >= 2
            && k.starts_with('F')
            && k[1..].chars().all(|c| c.is_ascii_digit())
            && k[1..]
                .parse::<u32>()
                .map(|n| (1..=12).contains(&n))
                .unwrap_or(false))
}

impl Accel {
    /// Parse an accelerator string; `None` when invalid.
    pub fn parse(s: &str) -> Option<Accel> {
        let s = s.trim();
        if s.is_empty() || s.len() > 100 {
            return None;
        }
        let mut accel = Accel {
            ctrl: false,
            shift: false,
            alt: false,
            super_key: false,
            key: String::new(),
        };
        let mut key_seen = false;
        for token in s.split(['<', '>']).filter(|t| !t.is_empty()) {
            if key_seen {
                return None; // key must come after modifiers, only once
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "primary" => accel.ctrl = true,
                "shift" => accel.shift = true,
                "alt" | "option" => accel.alt = true,
                "super" | "meta" | "win" | "cmd" => accel.super_key = true,
                _ => {
                    let mut chars = token.chars();
                    if let (Some(c), None) = (chars.next(), chars.next()) {
                        if c.is_ascii_alphanumeric() {
                            accel.key = c.to_ascii_lowercase().to_string();
                            key_seen = true;
                            continue;
                        }
                    }
                    if is_named_key(token) {
                        accel.key = token.to_string();
                        key_seen = true;
                        continue;
                    }
                    return None;
                }
            }
        }
        if !key_seen {
            return None;
        }
        // A bare printable key would eat global typing — reject it.
        let bare_printable = !accel.ctrl && !accel.alt && !accel.super_key && accel.key.len() == 1;
        if bare_printable {
            return None;
        }
        Some(accel)
    }
}

impl fmt::Display for Accel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "<Ctrl>")?;
        }
        if self.shift {
            write!(f, "<Shift>")?;
        }
        if self.alt {
            write!(f, "<Alt>")?;
        }
        if self.super_key {
            write!(f, "<Super>")?;
        }
        write!(f, "{}", self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_super_v() {
        let a = Accel::parse("<Super>v").unwrap();
        assert!(a.super_key && !a.ctrl);
        assert_eq!(a.key, "v");
        assert_eq!(a.to_string(), "<Super>v");
    }

    #[test]
    fn accepts_alias_modifiers_and_normalizes() {
        let a = Accel::parse("<Primary><Win>P").unwrap();
        assert!(a.ctrl && a.super_key);
        assert_eq!(a.to_string(), "<Ctrl><Super>p");
    }

    #[test]
    fn named_keys_parse_with_or_without_modifiers() {
        assert!(Accel::parse("F5").is_some());
        assert!(Accel::parse("<Ctrl>F5").is_some());
        assert!(Accel::parse("Delete").is_some());
    }

    #[test]
    fn rejects_bare_printable_key() {
        assert!(Accel::parse("v").is_none());
        assert!(Accel::parse("9").is_none());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Accel::parse("").is_none());
        assert!(Accel::parse("<Super>").is_none()); // no key
        assert!(Accel::parse("<Super>vx").is_none()); // multi-char key
        assert!(Accel::parse("v<Super>").is_none()); // modifier after key
        assert!(Accel::parse("<Hyper>x").is_none()); // unknown modifier
        assert!(Accel::parse("F99").is_none());
    }

    #[test]
    fn round_trips() {
        for s in ["<Super>v", "<Ctrl><Shift>t", "<Alt>F4", "<Ctrl>F5"] {
            assert_eq!(Accel::parse(s).unwrap().to_string(), s);
        }
    }
}
