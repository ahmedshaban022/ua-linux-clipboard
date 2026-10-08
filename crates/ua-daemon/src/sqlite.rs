//! SQLite storage (Linux). rusqlite with the bundled SQLite, WAL mode,
//! forward-only migrations via `meta.schema_version` (spec §4).

use rusqlite::{params, Connection, OptionalExtension};

use ua_core::model::{Entry, EntryKind};
use ua_core::settings::Settings;
use ua_core::store::Store;

const SCHEMA_VERSION: i64 = 1;

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = SqliteStore { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta(
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS entries(
                 id INTEGER PRIMARY KEY,
                 kind TEXT NOT NULL,
                 text TEXT,
                 html TEXT,
                 image_blob BLOB,
                 thumb_blob BLOB,
                 uris_json TEXT,
                 source_app TEXT,
                 copied_at INTEGER NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0,
                 pin_order INTEGER,
                 preview TEXT NOT NULL);",
        )?;
        let current: Option<i64> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|v| v.parse().ok());
        if current.unwrap_or(0) < SCHEMA_VERSION {
            self.conn.execute(
                "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [SCHEMA_VERSION.to_string()],
            )?;
        }
        Ok(())
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
        let kind: String = row.get("kind")?;
        let uris_json: Option<String> = row.get("uris_json")?;
        Ok(Entry {
            id: row.get("id")?,
            kind: match kind.as_str() {
                "text" => EntryKind::Text,
                "rich_text" => EntryKind::RichText,
                "image" => EntryKind::Image,
                _ => EntryKind::Uris,
            },
            text: row.get("text")?,
            html: row.get("html")?,
            image: row.get("image_blob")?,
            thumb: row.get("thumb_blob")?,
            uris: uris_json.and_then(|j| serde_json::from_str(&j).ok()),
            source_app: row.get("source_app")?,
            copied_at: row.get("copied_at")?,
            pinned: row.get::<_, i64>("pinned")? != 0,
            pin_order: row.get("pin_order")?,
            preview: row.get("preview")?,
        })
    }
}

const LIST_ORDER: &str = "ORDER BY pinned DESC, pin_order ASC, copied_at DESC, id DESC";

fn entry_params(e: &Entry, id: Option<u64>) -> Vec<Box<dyn rusqlite::types::ToSql>> {
    let mut v: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(id) = id {
        v.push(Box::new(id as i64));
    }
    v.push(Box::new(match e.kind {
        EntryKind::Text => "text",
        EntryKind::RichText => "rich_text",
        EntryKind::Image => "image",
        EntryKind::Uris => "uris",
    }));
    v.push(Box::new(e.text.clone()));
    v.push(Box::new(e.html.clone()));
    v.push(Box::new(e.image.clone()));
    v.push(Box::new(e.thumb.clone()));
    v.push(Box::new(
        e.uris
            .as_ref()
            .map(|u| serde_json::to_string(u).unwrap_or_default()),
    ));
    v.push(Box::new(e.source_app.clone()));
    v.push(Box::new(e.copied_at as i64));
    v.push(Box::new(e.pinned as i64));
    v.push(Box::new(e.pin_order.map(|o| o as i64)));
    v.push(Box::new(e.preview.clone()));
    v
}

impl Store for SqliteStore {
    fn list(&self) -> Vec<Entry> {
        let sql = format!("SELECT * FROM entries {LIST_ORDER}");
        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map([], SqliteStore::row_to_entry);
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    fn get(&self, id: u64) -> Option<Entry> {
        self.conn
            .query_row("SELECT * FROM entries WHERE id = ?1", [id as i64], |row| {
                SqliteStore::row_to_entry(row)
            })
            .optional()
            .ok()
            .flatten()
    }

    fn insert(&mut self, entry: Entry) -> u64 {
        let ps = entry_params(&entry, None);
        let sql = "INSERT INTO entries (kind, text, html, image_blob, thumb_blob, uris_json, source_app, copied_at, pinned, pin_order, preview) \
                   VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)";
        let refs: Vec<&dyn rusqlite::types::ToSql> = ps.iter().map(|b| b.as_ref()).collect();
        match self.conn.execute(sql, refs.as_slice()) {
            Ok(_) => self.conn.last_insert_rowid() as u64,
            Err(e) => {
                eprintln!("[daemon] insert failed: {e}");
                0
            }
        }
    }

    fn bump(&mut self, id: u64, at_ms: u64) {
        let _ = self.conn.execute(
            "UPDATE entries SET copied_at = ?1 WHERE id = ?2",
            params![at_ms as i64, id as i64],
        );
    }

    fn set_pinned(&mut self, id: u64, pinned: bool) {
        let next: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(pin_order), 0) + 1 FROM entries",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);
        let _ = self.conn.execute(
            "UPDATE entries SET pinned = ?1, pin_order = CASE WHEN ?1 THEN ?3 ELSE NULL END WHERE id = ?2",
            params![pinned as i64, id as i64, next],
        );
    }

    fn delete(&mut self, id: u64) {
        let _ = self
            .conn
            .execute("DELETE FROM entries WHERE id = ?1", [id as i64]);
    }

    fn clear_unpinned(&mut self) {
        let _ = self
            .conn
            .execute("DELETE FROM entries WHERE pinned = 0", []);
    }

    fn count_unpinned(&self) -> usize {
        self.conn
            .query_row("SELECT COUNT(*) FROM entries WHERE pinned = 0", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(0) as usize
    }

    fn next_id(&mut self) -> u64 {
        self.conn
            .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM entries", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(1) as u64
    }
}

pub fn load_settings(path: &str) -> Settings {
    Connection::open(path)
        .ok()
        .and_then(|conn| {
            conn.query_row("SELECT value FROM meta WHERE key = 'settings'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
        })
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_settings(path: &str, settings: &Settings) {
    if let Ok(conn) = Connection::open(path) {
        if let Ok(json) = serde_json::to_string(settings) {
            let _ = conn.execute(
                "INSERT INTO meta(key, value) VALUES('settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [json],
            );
        }
    }
}
