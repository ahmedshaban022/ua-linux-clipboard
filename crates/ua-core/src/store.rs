//! Storage abstraction. `ua-daemon` provides SQLite; tests use `MemoryStore`.

use crate::ipc::EntrySummary;
use crate::model::Entry;

pub trait Store: Send {
    /// All entries, display order: pinned (by pin_order) first, then
    /// unpinned newest-first. The panel splits sections from the flag.
    ///
    /// Carries payloads — needed by the dedup policy. Clients that only
    /// render the list should call [`Store::summaries`] instead so blobs
    /// are never materialized (spec: panel opens < 150ms).
    fn list(&self) -> Vec<Entry>;
    /// Payload-free listing for UI/IPC. Stores backed by SQL override this
    /// to select metadata columns only; the default maps `list`.
    fn summaries(&self) -> Vec<EntrySummary> {
        self.list().iter().map(EntrySummary::from).collect()
    }
    fn get(&self, id: u64) -> Option<Entry>;
    /// Insert a fully-formed entry, assigning its id. Returns the id.
    fn insert(&mut self, entry: Entry) -> u64;
    /// Attach (or clear) a generated thumbnail after insert.
    fn set_thumb(&mut self, id: u64, thumb: Option<Vec<u8>>);
    /// Move an existing entry to the top of recency (re-copy).
    fn bump(&mut self, id: u64, at_ms: u64);
    fn set_pinned(&mut self, id: u64, pinned: bool);
    fn delete(&mut self, id: u64);
    /// "Clear all" semantics: remove every unpinned entry, keep pins.
    fn clear_unpinned(&mut self);
    fn count_unpinned(&self) -> usize;
    /// Allocate the next id (used by policy when constructing entries).
    fn next_id(&mut self) -> u64;
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Vec<Entry>,
    next: u64,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn list(&self) -> Vec<Entry> {
        let mut pinned: Vec<Entry> = self.entries.iter().filter(|e| e.pinned).cloned().collect();
        pinned.sort_by_key(|e| e.pin_order.unwrap_or(e.id));

        let mut unpinned: Vec<Entry> = self.entries.iter().filter(|e| !e.pinned).cloned().collect();
        unpinned.sort_by(|a, b| b.copied_at.cmp(&a.copied_at).then(b.id.cmp(&a.id)));

        pinned.extend(unpinned);
        pinned
    }

    fn get(&self, id: u64) -> Option<Entry> {
        self.entries.iter().find(|e| e.id == id).cloned()
    }

    fn insert(&mut self, mut entry: Entry) -> u64 {
        entry.id = self.next_id();
        let id = entry.id;
        self.entries.push(entry);
        id
    }

    fn set_thumb(&mut self, id: u64, thumb: Option<Vec<u8>>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.thumb = thumb;
        }
    }

    fn bump(&mut self, id: u64, at_ms: u64) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.copied_at = at_ms;
        }
    }

    fn set_pinned(&mut self, id: u64, pinned: bool) {
        // Inline the id bump so we don't double-borrow self while
        // iterating entries.
        self.next += 1;
        let order = self.next;
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.pinned = pinned;
            e.pin_order = if pinned { Some(order) } else { None };
        }
    }

    fn delete(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    fn clear_unpinned(&mut self) {
        self.entries.retain(|e| e.pinned);
    }

    fn count_unpinned(&self) -> usize {
        self.entries.iter().filter(|e| !e.pinned).count()
    }

    fn next_id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, EntryKind};

    fn entry(id: u64, copied_at: u64) -> Entry {
        Entry {
            id,
            kind: EntryKind::Text,
            text: Some(format!("e{id}")),
            html: None,
            image: None,
            thumb: None,
            uris: None,
            source_app: None,
            copied_at,
            pinned: false,
            pin_order: None,
            preview: format!("e{id}"),
        }
    }

    #[test]
    fn list_orders_pinned_first_then_newest() {
        let mut s = MemoryStore::new();
        s.entries = vec![entry(1, 100), entry(2, 300), entry(3, 200)];
        s.entries[0].pinned = true;
        s.entries[0].pin_order = Some(1);
        let ids: Vec<u64> = s.list().iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2, 3]); // pin, then 300, then 200
    }

    #[test]
    fn clear_unpinned_keeps_pins() {
        let mut s = MemoryStore::new();
        s.entries = vec![entry(1, 100), entry(2, 200)];
        s.entries[0].pinned = true;
        s.clear_unpinned();
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].id, 1);
    }

    #[test]
    fn bump_moves_recency() {
        let mut s = MemoryStore::new();
        s.entries = vec![entry(1, 100), entry(2, 200)];
        s.bump(1, 999);
        let ids: Vec<u64> = s.list().iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn summaries_and_set_thumb() {
        let mut s = MemoryStore::new();
        let id = s.insert(entry(1, 100));
        let sums = s.summaries();
        assert_eq!(sums.len(), 1);
        assert_eq!(sums[0].id, id);
        assert_eq!(sums[0].size_bytes, 2); // "e1"
        s.set_thumb(id, Some(vec![1, 2, 3]));
        assert_eq!(s.get(id).unwrap().thumb, Some(vec![1, 2, 3]));
        s.set_thumb(id, None);
        assert_eq!(s.get(id).unwrap().thumb, None);
    }
}
