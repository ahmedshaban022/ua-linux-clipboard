//! Wire types for the daemon's JSON-line IPC (Unix socket now; the DBus
//! `org.ua.Clipboard` mirror rides on these same types — see README
//! status table). Shared by ua-cli, ua-gtk, ua-ipc and the GNOME extension.

use serde::{Deserialize, Serialize};

use crate::model::{Entry, EntryKind};
use crate::settings::Settings;

/// What a watcher/extension pushes for one clipboard change. Mirrors
/// `RawCapture` minus the daemon-owned timestamp, with the image
/// base64-encoded for the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePayload {
    pub offers: Vec<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    /// Base64 PNG.
    pub image_b64: Option<String>,
    pub uris: Option<Vec<String>>,
    pub source_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    /// List summaries, newest first, optionally filtered by query
    /// (server-side full-text search) and paged by offset.
    List {
        query: String,
        offset: u32,
    },
    Get {
        id: u64,
    },
    /// Make this entry the system clipboard; daemon runs the paste pipeline.
    Select {
        id: u64,
    },
    /// Select but paste the plain-text representation only (panel context
    /// menu, ticket 07).
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
        capture: CapturePayload,
    },
    /// Health/diagnostics for `ua-clipboard doctor`.
    Status,
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
    Entries {
        entries: Vec<EntrySummary>,
    },
    Entry {
        entry: Option<Entry>,
    },
    Ok,
    Settings(Settings),
    Status {
        backend: String,
        captures: u64,
        secrets_skipped: u64,
        oversize_skipped: u64,
    },
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_over_json() {
        let req = Request::List {
            query: "foo".into(),
            offset: 0,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"list\""));
        let back: Request = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Request::List { query, offset: 0 } if query == "foo"));
    }

    #[test]
    fn capture_payload_round_trips() {
        let req = Request::Capture {
            capture: CapturePayload {
                offers: vec!["text/plain;charset=utf-8".into()],
                text: Some("hi".into()),
                html: None,
                image_b64: None,
                uris: None,
                source_app: None,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"capture\""));
        let back: Request = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Request::Capture { .. }));
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
