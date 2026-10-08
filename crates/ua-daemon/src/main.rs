//! UA Clipboard daemon: watches the clipboard, owns storage, serves IPC.
//!
//! Architecture (spec §2, ADR-0002): the daemon is the single source of
//! truth. Watchers push `RawCapture`s in; clients (panel, CLI, GNOME
//! extension) issue `Request`s over a Unix-socket JSON-line protocol
//! (the DBus mirror is a declared pending milestone — README status).

mod ipc;
#[cfg(target_os = "linux")]
mod sqlite;
mod watchers;

use std::sync::{Arc, Mutex};

use ua_core::clock::{Clock, SystemClock};
use ua_core::ipc::{CapturePayload, Request, Response};
use ua_core::model::RawCapture;
use ua_core::policy::{admit, Admission};
use ua_core::settings::Settings;
use ua_core::store::{MemoryStore, Store};

/// Counters surfaced via `Request::Status` for `ua-clipboard doctor`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub captures: u64,
    pub secrets_skipped: u64,
    pub oversize_skipped: u64,
}

/// State shared between the IPC server and the admission loop.
pub struct DaemonState {
    pub store: Box<dyn Store>,
    pub settings: Settings,
    /// Where settings persist (sqlite path on Linux; None in dev mode).
    pub db_path: Option<String>,
    pub backend: &'static str,
    pub stats: Stats,
}

fn main() {
    install_panic_logger();

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

    let backend = watchers::detect().label();
    let shortcut = loaded.shortcut.clone();
    let state = Arc::new(Mutex::new(DaemonState {
        store,
        settings: loaded,
        db_path: db_path_opt,
        backend,
        stats: Stats::default(),
    }));

    // Watcher → admission loop.
    let (tx, rx) = std::sync::mpsc::channel::<RawCapture>();
    watchers::spawn(tx, &shortcut);

    let admission_state = Arc::clone(&state);
    std::thread::spawn(move || {
        for capture in rx {
            let mut s = admission_state.lock().unwrap();
            let id = apply_capture(&mut s, &capture);
            if let Some(id) = id {
                let _ = generate_thumbnail(&mut s, id);
            }
        }
    });

    println!("ua-daemon: listening on {socket_path}");
    if let Err(e) = ipc::serve(&socket_path, state) {
        eprintln!("ua-daemon: IPC server failed: {e}");
        std::process::exit(1);
    }
}

/// Admit one capture, update stats, log once. Shared by the watcher loop
/// and the IPC `Capture` arm so the admission story lives in one place.
pub fn apply_capture(state: &mut DaemonState, capture: &RawCapture) -> Option<u64> {
    let policy = state.settings.history_policy();
    let outcome = admit(state.store.as_mut(), capture, &policy);
    match outcome {
        Admission::Added { id } | Admission::Bumped { id } => {
            state.stats.captures += 1;
            log_line(&format!("entry {id} admitted"));
            Some(id)
        }
        Admission::SkippedSecret => {
            state.stats.secrets_skipped += 1;
            log_line("skipped secret (password manager)");
            None
        }
        Admission::SkippedOversizeImage => {
            state.stats.oversize_skipped += 1;
            log_line("skipped oversize image");
            None
        }
        Admission::SkippedEmpty => None,
    }
}

/// Spec §4: thumbnails ≤ 96dp generated on insert (PNG in, PNG out).
fn generate_thumbnail(state: &mut DaemonState, id: u64) -> Option<Vec<u8>> {
    let image = state.store.get(id)?.image?;
    let thumb = thumbnail_png(&image)?;
    state.store.set_thumb(id, Some(thumb.clone()));
    Some(thumb)
}

#[cfg(target_os = "linux")]
fn thumbnail_png(image: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(image).ok()?;
    let thumb = img.thumbnail(96, 96);
    let mut out = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(not(target_os = "linux"))]
fn thumbnail_png(_image: &[u8]) -> Option<Vec<u8>> {
    None
}

/// Spec §8: panics land in ~/.local/state/ua-clipboard/daemon.log, not
/// just the journal.
fn install_panic_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let line = format!("[panic {}] {info}", SystemClock.now_ms());
        if let Some(path) = state_log_path() {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
        default_hook(info);
    }));
}

fn state_log_path() -> Option<String> {
    std::env::var("UA_CLIPBOARD_LOG").ok().or_else(|| {
        std::env::var("HOME")
            .ok()
            .map(|home| format!("{home}/.local/state/ua-clipboard/daemon.log"))
    })
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
        Request::List { query, offset } => {
            let all = state.store.summaries();
            let query = query.trim().to_string();
            let matches_query = |s: &ua_core::ipc::EntrySummary| -> bool {
                query.is_empty()
                    || state
                        .store
                        .get(s.id)
                        .map(|e| ua_core::search::matches(&e, &query))
                        .unwrap_or(false)
            };
            let rows: Vec<_> = all
                .into_iter()
                .filter(matches_query)
                .skip(offset as usize)
                .collect();
            Response::Entries { entries: rows }
        }
        Request::Status => Response::Status {
            backend: state.backend.to_string(),
            captures: state.stats.captures,
            secrets_skipped: state.stats.secrets_skipped,
            oversize_skipped: state.stats.oversize_skipped,
        },
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
        Request::Toggle => {
            spawn_panel();
            Response::Ok
        }
        Request::Capture { capture } => {
            let raw = capture_into_raw(capture);
            let id = apply_capture(state, &raw);
            if let Some(id) = id {
                let _ = generate_thumbnail(state, id);
            }
            Response::Ok
        }
    }
}

fn capture_into_raw(capture: CapturePayload) -> RawCapture {
    let image = capture.image_b64.as_deref().and_then(decode_b64);
    RawCapture {
        offers: capture.offers,
        text: capture.text,
        html: capture.html,
        image,
        uris: capture.uris,
        source_app: capture.source_app,
        at_ms: SystemClock.now_ms(),
    }
}

fn decode_b64(b64: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Super+V ends here on X11 (daemon owns the XGrabKey) and via the
/// shortcut command elsewhere: present the (single-instance) panel.
fn spawn_panel() {
    use std::process::{Command, Stdio};
    let _ = Command::new("ua-clipboard-panel")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn persist_settings(state: &DaemonState) {
    #[cfg(target_os = "linux")]
    if let Some(path) = &state.db_path {
        sqlite::save_settings(path, &state.settings);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = state;
}
