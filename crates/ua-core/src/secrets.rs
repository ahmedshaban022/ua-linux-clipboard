//! Secret detection: content marked sensitive by its source is never stored.
//!
//! KeePassXC (and other password managers following the Klipper convention)
//! offer an extra MIME format alongside the payload. Its presence means
//! "do not log me". The exact spelling is confirmed on real Linux during
//! implementation (see spec §5); matching is case-insensitive to be safe.

/// MIME offers that mark a clipboard source as sensitive.
pub const SECRET_MIME_MARKERS: &[&str] = &["x-kde-passwordManagerHint"];

pub fn is_secret(offers: &[String]) -> bool {
    offers.iter().any(|offer| {
        SECRET_MIME_MARKERS
            .iter()
            .any(|marker| offer.eq_ignore_ascii_case(marker))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offers(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_text_is_not_secret() {
        assert!(!is_secret(&offers(&[
            "text/plain;charset=utf-8",
            "text/html"
        ])));
    }

    #[test]
    fn password_manager_hint_marks_secret() {
        assert!(is_secret(&offers(&[
            "text/plain;charset=utf-8",
            "x-kde-passwordManagerHint"
        ])));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(is_secret(&offers(&["X-KDE-PASSWORDMANAGERHINT"])));
    }
}
