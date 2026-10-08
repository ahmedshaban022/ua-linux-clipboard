//! User settings with validation. Everything configurable, sane defaults.

use serde::{Deserialize, Serialize};

use crate::accel::Accel;

pub const DEFAULT_SHORTCUT: &str = "<Super>v";
pub const DEFAULT_MAX_ENTRIES: u32 = 100;
pub const DEFAULT_MAX_IMAGE_MB: u32 = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Accelerator string, see `accel::Accel`.
    pub shortcut: String,
    pub max_entries: u32,
    pub max_image_mb: u32,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shortcut: DEFAULT_SHORTCUT.to_string(),
            max_entries: DEFAULT_MAX_ENTRIES,
            max_image_mb: DEFAULT_MAX_IMAGE_MB,
            autostart: true,
        }
    }
}

impl Settings {
    /// Validate the whole set; Err carries a user-presentable message.
    pub fn validate(&self) -> Result<(), String> {
        match Accel::parse(&self.shortcut) {
            Some(_) => {}
            None => {
                return Err(format!(
                    "Invalid shortcut '{}': use e.g. <Super>v",
                    self.shortcut
                ))
            }
        }
        if !(10..=1000).contains(&self.max_entries) {
            return Err(format!(
                "History size must be 10–1000, got {}",
                self.max_entries
            ));
        }
        if !(1..=256).contains(&self.max_image_mb) {
            return Err(format!(
                "Image size cap must be 1–256 MB, got {}",
                self.max_image_mb
            ));
        }
        Ok(())
    }

    pub fn history_policy(&self) -> crate::policy::HistoryPolicy {
        crate::policy::HistoryPolicy {
            max_entries: self.max_entries,
            max_image_bytes: self.max_image_mb as usize * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn default_shortcut_is_super_v() {
        assert_eq!(Settings::default().shortcut, "<Super>v");
    }

    #[test]
    fn rejects_bare_key_shortcut_and_bad_ranges() {
        let s = Settings {
            shortcut: "x".into(),
            ..Default::default()
        };
        assert!(s.validate().is_err());
        let s = Settings {
            max_entries: 5,
            ..Default::default()
        };
        assert!(s.validate().is_err());
        let s = Settings {
            max_image_mb: 0,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn policy_derives_from_settings() {
        let s = Settings {
            max_entries: 50,
            max_image_mb: 8,
            ..Default::default()
        };
        let p = s.history_policy();
        assert_eq!(p.max_entries, 50);
        assert_eq!(p.max_image_bytes, 8 * 1024 * 1024);
    }
}
