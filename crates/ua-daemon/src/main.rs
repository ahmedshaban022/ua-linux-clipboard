//! UA Clipboard daemon: watches the clipboard, owns storage, serves IPC.
//!
//! Architecture (spec §2, ADR-0002): the daemon is the single source of
//! truth. Watchers push `RawCapture`s in; clients (panel, CLI, GNOME
//! extension) issue `Request`s over a Unix-socket JSON-line protocol.
//!
//! Platform notes:
//! - Linux: SQLite storage, real watchers (see `watchers`).
//! - Other OSes (development only): in-memory store, stdin capture —
//!   enough to exercise the IPC protocol end-to-end on any host.

mod ipc;
mod ipc_client;
#[cfg(target_os = "linux")]
mod sqlite;
mod watchers;

use std::sync::{Arc, Mutex};

use ua_core::clock::{Clock, SystemClock};
use ua_core::ipc::{EntrySummary, Request, Response};
use ua_core::model::RawCapture;
use ua_core::policy::{admit, Admission};
use ua_core::settings::Settings;
use ua_core::store::{MemoryStore, Store};

/// State shared between the IPC server and the admission loop.
pub struct DaemonState {
    pub store: Box<dyn Store>,
    pub settings: Settings,
    /// Where settings persist (sqlite path on Linux; None in dev mode).
    pub db_path: Option<String>,
}

fn main() {
    // wl-paste --watch hook: relay one capture from stdin to the daemon.
    if std::env::args().any(|a| a == "--capture-stdin") {
        std::process::exit(watchers::capture_stdin_main());
    }

    let db_path = db_path_default();
    let socket_path = socket_path();

    #[cfg(target_os = "linux")]
    let (store, loaded, db_path_opt) = {
        if let Some(parent) = std::path::Path::new(&db_path).parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("ua-daemon: cannot create state dir: {e}");
            }
        }
        match sqlite::SqliteStore::open(&db_path) {
            Ok(s) => (
                Box::new(s) as Box<dyn Store>,
                sqlite::load_settings(&db_path),
                Some(db_path.clone()),
            ),
            Err(e) => {
                eprintln!("ua-daemon: sqlite open failed ({e}); falling back to in-memory");
                (
                    Box::new(MemoryStore::new()) as Box<dyn Store>,
                    Settings::default(),
                    None,
                )
            }
        }
    };
    #[cfg(not(target_os = "linux"))]
    let (store, loaded, db_path_opt) = (
        Box::new(MemoryStore::new()) as Box<dyn Store>,
        Settings::default(),
        None,
    );

    let state = Arc::new(Mutex::new(DaemonState {
        store,
        settings: loaded,
        db_path: db_path_opt,
    }));

    // Watcher → admission loop.
    let (tx, rx) = std::sync::mpsc::channel::<RawCapture>();
    watchers::spawn(tx);

    let admission_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let clock = SystemClock;
        for capture in rx {
            let mut s = admission_state.lock().unwrap();
            let policy = s.settings.history_policy();
            let outcome = admit(s.store.as_mut(), &capture, &policy);
            drop(s);
            match outcome {
                Admission::Added { id } => log_line(&format!("added entry {id}")),
                Admission::Bumped { id } => log_line(&format!("bumped entry {id}")),
                Admission::SkippedSecret => log_line("skipped secret (password manager)"),
                Admission::SkippedOversizeImage => log_line("skipped oversize image"),
                Admission::SkippedEmpty => {}
            }
            let _ = clock.now_ms();
        }
    });

    println!("ua-daemon: listening on {socket_path}");
    if let Err(e) = ipc::serve(&socket_path, state) {
        eprintln!("ua-daemon: IPC server failed: {e}");
        std::process::exit(1);
    }
}

pub fn socket_path() -> String {
    std::env::var("UA_CLIPBOARD_SOCKET").unwrap_or_else(|_| {
        let runtime = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()));
        format!("{runtime}/ua-clipboard.sock")
    })
}

pub fn db_path_default() -> String {
    std::env::var("UA_CLIPBOARD_DB").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.local/state/ua-clipboard/history.db")
    })
}

fn log_line(msg: &str) {
    println!("[daemon] {msg}");
}

/// Handle one client request. Kept here so ipc.rs stays transport-only.
pub fn handle_request(state: &mut DaemonState, req: Request) -> Response {
    match req {
        Request::Ping => Response::Pong,
        Request::List { query } => {
            let rows: Vec<EntrySummary> = state
                .store
                .list()
                .iter()
                .filter(|e| ua_core::search::matches(e, &query))
                .map(EntrySummary::from)
                .collect();
            Response::Entries { items: rows }
        }
        Request::Get { id } => Response::Entry {
            entry: state.store.get(id),
        },
        Request::Pin { id, pinned } => {
            state.store.set_pinned(id, pinned);
            Response::Ok
        }
        Request::Delete { id } => {
            state.store.delete(id);
            Response::Ok
        }
        Request::ClearAll => {
            state.store.clear_unpinned();
            Response::Ok
        }
        Request::GetSettings => Response::Settings(state.settings.clone()),
        Request::SetSettings { settings } => match settings.validate() {
            Ok(()) => {
                state.settings = settings;
                persist_settings(state);
                Response::Ok
            }
            Err(e) => Response::Error(e),
        },
        Request::Select { id } | Request::SelectPlainText { id } => {
            // Transitional (README status table): selection ownership and
            // Ctrl+V injection land with the platform integration work.
            // The pick is validated and acknowledged for now.
            match state.store.get(id) {
                Some(_) => {
                    log_line("select: paste requested (platform paste pending)");
                    Response::Ok
                }
                None => Response::Error(format!("no entry {id}")),
            }
        }
        Request::Toggle => Response::Ok,
        Request::Capture {
            offers,
            text,
            html,
            image_b64,
            uris,
            source_app,
        } => {
            let image = image_b64.as_deref().and_then(decode_b64);
            let capture = RawCapture {
                offers,
                text,
                html,
                image,
                uris,
                source_app,
                at_ms: SystemClock.now_ms(),
            };
            let policy = state.settings.history_policy();
            match admit(state.store.as_mut(), &capture, &policy) {
                Admission::Added { id } | Admission::Bumped { id } => {
                    log_line(&format!("capture → entry {id}"))
                }
                Admission::SkippedSecret => log_line("capture skipped: secret"),
                Admission::SkippedOversizeImage => log_line("capture skipped: oversize image"),
                Admission::SkippedEmpty => {}
            }
            Response::Ok
        }
    }
}

fn decode_b64(b64: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

fn persist_settings(state: &DaemonState) {
    #[cfg(target_os = "linux")]
    if let Some(path) = &state.db_path {
        sqlite::save_settings(path, &state.settings);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = state;
}
