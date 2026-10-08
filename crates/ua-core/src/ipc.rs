//! Wire types for the daemon's JSON-line IPC (Unix socket now; DBus mirror
//! per spec §2 later). Shared by ua-cli, ua-gtk and the GNOME extension.

use serde::{Deserialize, Serialize};

use crate::model::{Entry, EntryKind};
use crate::settings::Settings;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    /// Full list, optionally filtered by query (server-side search).
    List {
        query: String,
    },
    Get {
        id: u64,
    },
    /// Make this entry the system clipboard; daemon runs the paste pipeline.
    Select {
        id: u64,
    },
    /// Select but paste the plain-text representation only.
    SelectPlainText {
        id: u64,
    },
    Pin {
        id: u64,
        pinned: bool,
    },
    Delete {
        id: u64,
    },
    /// Remove every unpinned entry.
    ClearAll,
    GetSettings,
    SetSettings {
        settings: Settings,
    },
    /// Toggle the panel (sent by the shortcut command).
    Toggle,
    /// A capture pushed by a watcher (the GNOME extension pushes here).
    Capture {
        offers: Vec<String>,
        text: Option<String>,
        html: Option<String>,
        /// Base64 PNG.
        image_b64: Option<String>,
        uris: Option<Vec<String>>,
        source_app: Option<String>,
    },
}

/// List rows carry no payloads — the panel fetches blobs via `Get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySummary {
    pub id: u64,
    pub kind: EntryKind,
    pub preview: String,
    pub pinned: bool,
    pub copied_at: u64,
    pub source_app: Option<String>,
    pub size_bytes: u64,
}

impl From<&Entry> for EntrySummary {
    fn from(e: &Entry) -> Self {
        let size_bytes = e.text.as_ref().map(|t| t.len()).unwrap_or(0) as u64
            + e.html.as_ref().map(|h| h.len()).unwrap_or(0) as u64
            + e.image.as_ref().map(|i| i.len()).unwrap_or(0) as u64;
        EntrySummary {
            id: e.id,
            kind: e.kind,
            preview: e.preview.clone(),
            pinned: e.pinned,
            copied_at: e.copied_at,
            source_app: e.source_app.clone(),
            size_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    Entries { items: Vec<EntrySummary> },
    Entry { entry: Option<Entry> },
    Ok,
    Settings(Settings),
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_over_json() {
        let req = Request::List {
            query: "foo".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"list\""));
        let back: Request = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Request::List { query } if query == "foo"));
    }

    #[test]
    fn summary_excludes_payloads() {
        let e = Entry {
            id: 1,
            kind: EntryKind::Image,
            text: None,
            html: None,
            image: Some(vec![0u8; 4096]),
            thumb: None,
            uris: None,
            source_app: Some("gimp".into()),
            copied_at: 42,
            pinned: true,
            pin_order: Some(1),
            preview: "Image · 4.0 KB".into(),
        };
        let s = EntrySummary::from(&e);
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("AAAA"));
        assert_eq!(s.size_bytes, 4096);
    }
}
