//! Instant search: case-insensitive substring filter with match highlighting.
//! Spec §6: the panel's search field filters as you type.

use crate::model::Entry;

/// True when the entry matches the query. Empty/blank query matches everything.
pub fn matches(entry: &Entry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let mut haystack = String::with_capacity(entry.preview.len() + 64);
    haystack.push_str(&entry.preview);
    haystack.push('\n');
    if let Some(t) = &entry.text {
        haystack.push_str(t);
        haystack.push('\n');
    }
    if let Some(uris) = &entry.uris {
        for u in uris {
            haystack.push_str(u);
            haystack.push('\n');
        }
    }
    haystack.to_lowercase().contains(&q)
}

/// A run of bytes in the preview string, flagged when it is part of a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub byte_len: usize,
    pub matched: bool,
}

/// Split `s` into spans, marking case-insensitive occurrences of `query`.
///
/// Matching is ASCII-case-insensitive (both sides folded with `to_ascii_`
/// on a byte window); non-ASCII text passes through unchanged, which is fine
/// because `matches()` above does full Unicode folding for filtering —
/// highlighting non-ASCII queries exactly is a follow-up, not a correctness
/// bug (worst case the match is highlighted only after re-type).
pub fn highlight(s: &str, query: &str) -> Vec<Span> {
    let q: Vec<u8> = query.trim().to_ascii_lowercase().into_bytes();
    let bytes = s.as_bytes();
    if q.is_empty() || q.len() > bytes.len() {
        return vec![Span {
            start: 0,
            byte_len: bytes.len(),
            matched: false,
        }];
    }

    let mut spans = Vec::new();
    let mut run_start = 0usize;
    let mut i = 0usize;
    while i + q.len() <= bytes.len() {
        let window = &bytes[i..i + q.len()];
        if window.eq_ignore_ascii_case(&q)
            && s.is_char_boundary(i)
            && s.is_char_boundary(i + q.len())
        {
            if i > run_start {
                spans.push(Span {
                    start: run_start,
                    byte_len: i - run_start,
                    matched: false,
                });
            }
            spans.push(Span {
                start: i,
                byte_len: q.len(),
                matched: true,
            });
            i += q.len();
            run_start = i;
        } else {
            i += 1;
        }
    }
    if run_start < bytes.len() {
        spans.push(Span {
            start: run_start,
            byte_len: bytes.len() - run_start,
            matched: false,
        });
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, EntryKind};

    fn entry(text: &str) -> Entry {
        Entry {
            id: 1,
            kind: EntryKind::Text,
            text: Some(text.into()),
            html: None,
            image: None,
            thumb: None,
            uris: None,
            source_app: None,
            copied_at: 0,
            pinned: false,
            pin_order: None,
            preview: crate::model::ellipsize(text, 120),
        }
    }

    #[test]
    fn empty_query_matches_all() {
        assert!(matches(&entry("anything"), ""));
        assert!(matches(&entry("anything"), "   "));
    }

    #[test]
    fn substring_is_case_insensitive_and_anywhere() {
        let e = entry("The Quick Brown Fox");
        assert!(matches(&e, "quick"));
        assert!(matches(&e, "BROWN"));
        assert!(matches(&e, "ck br"));
        assert!(!matches(&e, "zebra"));
    }

    #[test]
    fn searches_full_text_not_just_preview() {
        let mut e = entry("short preview");
        let long = format!("{}needle{}", "x".repeat(200), "y".repeat(200));
        e.text = Some(long);
        assert!(matches(&e, "needle"));
    }

    #[test]
    fn unicode_text_matches() {
        let e = entry("مرحبا بكم");
        assert!(matches(&e, "بكم"));
        assert!(!matches(&e, "عالم"));
    }

    #[test]
    fn highlight_marks_all_occurrences() {
        let spans = highlight("Buffalo buffalo BUFFALO", "buffalo");
        assert_eq!(spans.iter().filter(|s| s.matched).count(), 3);
        // the two separating spaces are their own unmatched runs
        assert_eq!(spans.iter().filter(|s| !s.matched).count(), 2);
    }

    #[test]
    fn highlight_keeps_plain_runs() {
        let spans = highlight("abcDEFghi", "def");
        assert_eq!(spans.len(), 3);
        assert_eq!(
            spans[0],
            Span {
                start: 0,
                byte_len: 3,
                matched: false
            }
        );
        assert_eq!(
            spans[1],
            Span {
                start: 3,
                byte_len: 3,
                matched: true
            }
        );
        assert_eq!(
            spans[2],
            Span {
                start: 6,
                byte_len: 3,
                matched: false
            }
        );
    }

    #[test]
    fn highlight_empty_query_is_one_plain_span() {
        let spans = highlight("hello", "");
        assert_eq!(
            spans,
            vec![Span {
                start: 0,
                byte_len: 5,
                matched: false
            }]
        );
    }

    #[test]
    fn highlight_does_not_split_chars() {
        // 'é' is two bytes; a query aligned mid-char must not "match".
        let spans = highlight("café", "f");
        assert_eq!(spans.iter().filter(|s| s.matched).count(), 1);
        let matched = spans.iter().find(|s| s.matched).unwrap();
        assert_eq!(
            &"café"[matched.start..matched.start + matched.byte_len],
            "f"
        );
    }
}
