//! History admission policy: dedup, bump-to-top, caps, eviction, secrets.
//!
//! Mirrors Windows 11 semantics with our improvements (spec §5):
//! consecutive or repeated copies of equal content never duplicate — the
//! existing entry is bumped to the top with a refreshed timestamp.

use crate::model::{Entry, RawCapture};
use crate::secrets::is_secret;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPolicy {
    pub max_entries: u32,
    pub max_image_bytes: usize,
}

impl Default for HistoryPolicy {
    fn default() -> Self {
        HistoryPolicy {
            max_entries: 100,
            max_image_bytes: 16 * 1024 * 1024, // 16 MB, spec §4
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    Added { id: u64 },
    Bumped { id: u64 },
    SkippedSecret,
    SkippedOversizeImage,
    SkippedEmpty,
}

/// Decide what a new capture does to history, and do it.
pub fn admit(store: &mut dyn Store, capture: &RawCapture, policy: &HistoryPolicy) -> Admission {
    if is_secret(&capture.offers) {
        return Admission::SkippedSecret;
    }

    let kind = capture.kind();
    if kind == crate::model::EntryKind::Image {
        let size = capture.image.as_ref().map(|v| v.len()).unwrap_or(0);
        if size > policy.max_image_bytes {
            return Admission::SkippedOversizeImage;
        }
    }

    let empty = capture.text.as_deref().unwrap_or("").is_empty()
        && capture.html.is_none()
        && capture.image.is_none()
        && capture.uris.as_deref().unwrap_or(&[]).is_empty();
    if empty {
        return Admission::SkippedEmpty;
    }

    let candidate = Entry {
        id: 0,
        kind,
        text: capture.text.clone(),
        html: capture.html.clone(),
        image: capture.image.clone(),
        thumb: None,
        uris: capture.uris.clone(),
        source_app: capture.source_app.clone(),
        copied_at: capture.at_ms,
        pinned: false,
        pin_order: None,
        preview: capture.preview(),
    };

    // Content-equal entry anywhere in history → bump it (Win11 behavior).
    if let Some(existing) = store
        .list()
        .into_iter()
        .find(|e| e.same_content(&candidate))
    {
        store.bump(existing.id, capture.at_ms);
        return Admission::Bumped { id: existing.id };
    }

    let id = store.insert(candidate);

    // Evict oldest unpinned entries beyond the cap. Pins never evicted.
    while store.count_unpinned() > policy.max_entries as usize {
        let oldest = store
            .list()
            .into_iter()
            .filter(|e| !e.pinned)
            .min_by_key(|e| (e.copied_at, e.id))
            .map(|e| e.id);
        match oldest {
            Some(oldest_id) => store.delete(oldest_id),
            None => break,
        }
    }

    Admission::Added { id }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    fn img(n: usize) -> Vec<u8> {
        vec![0u8; n]
    }

    #[test]
    fn secret_capture_is_never_stored() {
        let mut s = MemoryStore::new();
        let mut c = RawCapture::text(1, "hunter2");
        c.offers = vec!["x-kde-passwordManagerHint".into()];
        assert_eq!(
            admit(&mut s, &c, &Default::default()),
            Admission::SkippedSecret
        );
        assert_eq!(s.list().len(), 0);
    }

    #[test]
    fn oversize_image_skipped() {
        let mut s = MemoryStore::new();
        let mut c = RawCapture::text(1, "");
        c.text = None;
        c.image = Some(img(17 * 1024 * 1024));
        assert_eq!(
            admit(&mut s, &c, &Default::default()),
            Admission::SkippedOversizeImage
        );
    }

    #[test]
    fn empty_capture_skipped() {
        let mut s = MemoryStore::new();
        let c = RawCapture::text(1, "");
        assert_eq!(
            admit(&mut s, &c, &Default::default()),
            Admission::SkippedEmpty
        );
    }

    #[test]
    fn identical_recopy_bumps_instead_of_duplicating() {
        let mut s = MemoryStore::new();
        admit(&mut s, &RawCapture::text(100, "hello"), &Default::default());
        admit(&mut s, &RawCapture::text(200, "other"), &Default::default());
        let res = admit(&mut s, &RawCapture::text(300, "hello"), &Default::default());
        assert!(matches!(res, Admission::Bumped { .. }));
        assert_eq!(s.list().len(), 2);
        // bumped entry is now most recent
        assert_eq!(s.list()[0].preview, "hello");
        assert_eq!(s.list()[0].copied_at, 300);
    }

    #[test]
    fn cap_evicts_oldest_unpinned_and_never_pins() {
        let mut s = MemoryStore::new();
        let policy = HistoryPolicy {
            max_entries: 3,
            ..Default::default()
        };
        for i in 0..3 {
            admit(&mut s, &RawCapture::text(i, format!("v{i}")), &policy);
        }
        s.set_pinned(1, true); // pin v0, the oldest
        admit(&mut s, &RawCapture::text(90, "new1"), &policy);
        admit(&mut s, &RawCapture::text(99, "new2"), &policy);
        let list = s.list();
        assert!(list.iter().any(|e| e.id == 1 && e.pinned)); // pin survived
        assert!(!list.iter().any(|e| e.preview == "v1")); // oldest unpinned evicted
        assert_eq!(s.count_unpinned(), 3); // cap enforced on unpinned only
    }

    #[test]
    fn html_and_text_payloads_are_distinct_entries() {
        let mut s = MemoryStore::new();
        admit(&mut s, &RawCapture::text(1, "bold"), &Default::default());
        let mut c = RawCapture::text(2, "bold");
        c.html = Some("<b>bold</b>".into());
        admit(&mut s, &c, &Default::default());
        assert_eq!(s.list().len(), 2);
    }
}
