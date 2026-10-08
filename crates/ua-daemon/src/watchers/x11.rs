//! X11 watching: XFixes selection events drive everything (event-driven,
//! no polling — spec §1). On each SET_SELECTION_OWNER notification we fetch
//! the payload. Payload fetching is currently transitional via `xclip`
//! (ubiquitous, battle-tested); the native convert_selection read is the
//! next platform milestone tracked in the README status table. The event
//! source itself is native x11rb — we never poll.

use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use ua_core::clock::{Clock as _, SystemClock};
use ua_core::model::RawCapture;

use x11rb::connection::Connection;
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, CreateWindowAux, WindowClass};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

pub fn watch(tx: Sender<RawCapture>) {
    if let Err(e) = run(&tx) {
        eprintln!("[daemon] x11 watcher stopped: {e}");
    }
}

fn run(tx: &Sender<RawCapture>) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_nr) = RustConnection::connect(None)?;
    let screen = conn.setup().roots.get(screen_nr).ok_or("no screen")?;

    let win = conn.generate_id()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        win,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new(),
    )?;

    // Negotiate xfixes (any version ≥ 1 has selection events).
    let _version = conn.xfixes_query_version(5, 0)?.reply()?;

    let clipboard = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
    let mask = x11rb::protocol::xfixes::SelectionEventMask::SET_SELECTION_OWNER
        | x11rb::protocol::xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
        | x11rb::protocol::xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
    conn.xfixes_select_selection_input(win, clipboard, mask)?;
    conn.flush()?;

    println!("[daemon] x11: watching CLIPBOARD via XFixes");
    loop {
        let event = conn.wait_for_event()?;
        if let Event::XfixesSelectionNotify(notify) = event {
            if notify.selection == clipboard
                && notify.subtype == x11rb::protocol::xfixes::SelectionEvent::SET_SELECTION_OWNER
            {
                if let Some(capture) = fetch_capture() {
                    let _ = tx.send(capture);
                }
            }
        }
    }
}

/// Transitional payload fetch: ask `xclip` for the targets list (secret
/// markers) and the UTF-8 text. Images/HTML arrive with the native reader.
fn fetch_capture() -> Option<RawCapture> {
    let offers = Command::new("xclip")
        .args(["-o", "-selection", "clipboard", "-t", "TARGETS"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let text = Command::new("xclip")
        .args([
            "-o",
            "-selection",
            "clipboard",
            "-t",
            "text/plain;charset=utf-8",
        ])
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim_end_matches('\0')
                .to_string()
        });

    if offers.is_empty() && text.as_deref().unwrap_or("").is_empty() {
        return None;
    }

    Some(RawCapture {
        offers,
        text,
        html: None,
        image: None,
        uris: None,
        source_app: None,
        at_ms: SystemClock.now_ms(),
    })
}
