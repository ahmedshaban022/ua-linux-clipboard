//! The clipboard data model: what a watcher delivers, and what history stores.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Text,
    RichText,
    Image,
    Uris,
}

/// One stored clipboard item. See GLOSSARY.md: "Entry".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub kind: EntryKind,
    pub text: Option<String>,
    pub html: Option<String>,
    pub image: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
    pub uris: Option<Vec<String>>,
    pub source_app: Option<String>,
    /// Unix epoch milliseconds.
    pub copied_at: u64,
    pub pinned: bool,
    /// Ordering within the Pinned section; assigned when pinned.
    pub pin_order: Option<u64>,
    /// One or two line human preview rendered by the panel.
    pub preview: String,
}

impl Entry {
    /// Byte-equality of the payload, ignoring metadata: used for dedup.
    pub fn same_content(&self, other: &Entry) -> bool {
        self.kind == other.kind
            && self.text == other.text
            && self.html == other.html
            && self.image == other.image
            && self.uris == other.uris
    }
}

/// What a watcher hands the daemon for every clipboard change.
#[derive(Debug, Clone, PartialEq)]
pub struct RawCapture {
    /// MIME types the clipboard source offered (secret detection reads these).
    pub offers: Vec<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub image: Option<Vec<u8>>,
    pub uris: Option<Vec<String>>,
    pub source_app: Option<String>,
    /// Unix epoch milliseconds.
    pub at_ms: u64,
}

impl RawCapture {
    pub fn text(at_ms: u64, s: impl Into<String>) -> Self {
        RawCapture {
            offers: vec![],
            text: Some(s.into()),
            html: None,
            image: None,
            uris: None,
            source_app: None,
            at_ms,
        }
    }
}

pub fn kind_of(c: &RawCapture) -> EntryKind {
    if c.image.is_some() {
        EntryKind::Image
    } else if c.uris.is_some() {
        EntryKind::Uris
    } else if c.html.is_some() {
        EntryKind::RichText
    } else {
        EntryKind::Text
    }
}

/// Truncate to at most `max_chars` characters on a char boundary, with ellipsis.
pub fn ellipsize(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// Strip HTML tags crudely — good enough for a plain-text preview card.
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn preview_for(c: &RawCapture) -> String {
    match kind_of(c) {
        EntryKind::Image => {
            let kb = c.image.as_ref().map(|v| v.len()).unwrap_or(0) as f64 / 1024.0;
            ellipsize(&format!("Image · {kb:.1} KB"), 120)
        }
        EntryKind::Uris => {
            let uris = c.uris.as_deref().unwrap_or(&[]);
            if uris.is_empty() {
                "Links".to_string()
            } else if uris.len() == 1 {
                ellipsize(&uris[0], 120)
            } else {
                ellipsize(&format!("{} links · {}", uris.len(), uris[0]), 120)
            }
        }
        EntryKind::RichText => {
            let plain = c.html.as_deref().map(strip_tags).unwrap_or_default();
            ellipsize(&plain, 120)
        }
        EntryKind::Text => ellipsize(c.text.as_deref().unwrap_or(""), 120),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_priority_is_image_uris_html_text() {
        let mut c = RawCapture::text(0, "hi");
        assert_eq!(kind_of(&c), EntryKind::Text);
        c.html = Some("<b>hi</b>".into());
        assert_eq!(kind_of(&c), EntryKind::RichText);
        c.uris = Some(vec!["file:///x".into()]);
        assert_eq!(kind_of(&c), EntryKind::Uris);
        c.image = Some(vec![1, 2, 3]);
        assert_eq!(kind_of(&c), EntryKind::Image);
    }

    #[test]
    fn ellipsize_is_char_safe() {
        assert_eq!(ellipsize("hello", 5), "hello");
        assert_eq!(ellipsize("hello world", 5), "hell…");
        // Arabic is multi-byte in UTF-8; must not panic or split a char.
        let ar = "مرحبا بكم في العالم";
        let cut = ellipsize(ar, 6);
        assert!(cut.chars().count() <= 6 && cut.ends_with('…'));
    }

    #[test]
    fn strip_tags_and_collapse_space() {
        assert_eq!(strip_tags("<b>Hello</b> <i>world</i>"), "Hello world");
        assert_eq!(strip_tags("a<br/>b"), "ab");
    }

    #[test]
    fn previews_per_kind() {
        let mut c = RawCapture::text(0, "plain text");
        assert_eq!(preview_for(&c), "plain text");
        c.html = Some("<p>rich &amp; bold</p>".into());
        assert!(preview_for(&c).contains("rich"));
        c.uris = Some(vec!["file:///a".into(), "file:///b".into()]);
        assert!(preview_for(&c).starts_with("2 links"));
        c.uris = None;
        c.image = Some(vec![0u8; 2048]);
        assert_eq!(preview_for(&c), "Image · 2.0 KB");
    }
}
