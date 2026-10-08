//! i18n seam. Every user-facing string routes through `t()` so translation
//! (gettext) can be wired in ua-gtk without touching call sites (spec §9).
//! English ships first; Arabic (RTL) follows — GTK4 mirrors automatically.

/// Translate a source string. Identity for now; ua-gtk swaps in gettext.
pub fn t(s: &str) -> &str {
    s
}

#[macro_export]
macro_rules! t {
    ($s:expr) => {
        $crate::i18n::t($s)
    };
}
