//! UA Clipboard — pure domain logic.
//!
//! Everything here is OS-free and UI-free so it stays unit-testable on any
//! host. Environment-specific work (clipboard watching, input injection,
//! storage engines) lives in `ua-daemon` behind the traits defined here.

pub mod accel;
pub mod clock;
pub mod i18n;
pub mod ipc;
pub mod model;
pub mod paste;
pub mod policy;
pub mod search;
pub mod secrets;
pub mod settings;
pub mod store;

pub use accel::Accel;
pub use clock::{Clock, FakeClock, SystemClock};
pub use ipc::{EntrySummary, Request, Response};
pub use model::{Entry, EntryKind, RawCapture};
pub use paste::{PasteOutcome, PastePolicy, PastePorts};
pub use policy::{Admission, HistoryPolicy};
pub use settings::Settings;
pub use store::{MemoryStore, Store};
