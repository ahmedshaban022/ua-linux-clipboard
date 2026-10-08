//! X11 watching: XFixes selection events drive everything (event-driven,
//! no polling — spec §1). On each SET_SELECTION_OWNER notification we fetch
//! the payload. Payload fetching is currently transitional via `xclip`
//! (ubiquitous, battle-tested); the native convert_selection read is the
//! next platform milestone tracked in the README status table. The event
//! source itself is native x11rb — we never poll.
//!
//! This backend also owns the Super+V XGrabKey (spec §1 shortcut column
//! for X11): grabbed with the NumLock/CapsLock/ScrollLock variants so the
//! modifier state doesn't break it, and each press presents the panel.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;

use super::spawn_panel;
use ua_core::accel::Accel;
use ua_core::clock::{Clock as _, SystemClock};
use ua_core::model::RawCapture;

use x11rb::connection::Connection;
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::{
    ConnectionExt as _, CreateWindowAux, GrabMode, Keycode, ModMask, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

pub fn watch(tx: Sender<RawCapture>, shortcut: &str) {
    if let Err(e) = run(&tx, shortcut) {
        eprintln!("[daemon] x11 watcher stopped: {e}");
    }
}

fn run(tx: &Sender<RawCapture>, shortcut: &str) -> Result<(), Box<dyn std::error::Error>> {
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

    // Global shortcut: resolve the configured accel to (mods, keycode)
    // ONCE — the same pair drives both the XGrabKey and the KeyPress match,
    // so the grabbed key and the handled key can never diverge.
    let shortcut_grab = resolve_shortcut(&conn, shortcut);
    match shortcut_grab {
        Some((base, keycode)) => grab_key_combo(&conn, screen.root, base, keycode)?,
        None => eprintln!(
            "[daemon] x11: shortcut '{shortcut}' not grabbable (unmapped key?); global key disabled"
        ),
    }

    conn.flush()?;

    println!("[daemon] x11: watching CLIPBOARD via XFixes; shortcut grabbed");
    let last_panel = AtomicU32::new(0);
    let pressed_key = shortcut_grab.map(|(_, keycode)| keycode);
    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::XfixesSelectionNotify(notify) => {
                if notify.selection == clipboard
                    && notify.subtype
                        == x11rb::protocol::xfixes::SelectionEvent::SET_SELECTION_OWNER
                {
                    if let Some(capture) = fetch_capture() {
                        let _ = tx.send(capture);
                    }
                }
            }
            Event::KeyPress(press)
                if pressed_key == Some(press.detail) && debounce_ok(&last_panel, press.time) =>
            {
                spawn_panel();
            }
            _ => {}
        }
    }
}

/// Parse the settings accelerator into (modifier bits, keysym).
fn shortcut_key(shortcut: &str) -> Option<(ModMask, u32)> {
    let accel = Accel::parse(shortcut).or_else(|| Accel::parse("<Super>v"))?;
    let mut mods = ModMask::default();
    if accel.ctrl {
        mods |= ModMask::CONTROL;
    }
    if accel.shift {
        mods |= ModMask::SHIFT;
    }
    if accel.alt {
        mods |= ModMask::M1;
    }
    if accel.super_key {
        mods |= ModMask::M4;
    }
    let sym = keysym(&accel.key)?;
    Some((mods, sym))
}

fn keysym(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let c = key.chars().next()?;
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase() as u32);
        }
        return None;
    }
    // F1 = 0xffbe … F12 = 0xffc9
    if let Some(num) = key.strip_prefix('F') {
        if let Ok(n) = num.parse::<u32>() {
            if (1..=12).contains(&n) {
                return Some(0xffbe + n - 1);
            }
        }
    }
    None
}

/// Resolve the shortcut to (modifier bits, keycode) on this keyboard, or
/// None if the key isn't mapped (grab disabled, logged by the caller).
fn resolve_shortcut(conn: &RustConnection, shortcut: &str) -> Option<(ModMask, Keycode)> {
    let (mods, sym) = shortcut_key(shortcut)?;
    let keycode = keycode_for_keysym(conn, sym)?;
    Some((mods, keycode))
}

/// Grab `keycode` with `base` modifiers on the root window, once per
/// NumLock/CapsLock/ScrollLock combination so the grab survives modifier
/// state (the classic X11 gotcha, ticket 02).
fn grab_key_combo(
    conn: &RustConnection,
    root: u32,
    base: ModMask,
    keycode: Keycode,
) -> Result<(), Box<dyn std::error::Error>> {
    let numlock = ModMask::M2;
    let capslock = ModMask::LOCK;
    let scrolllock = ModMask::M5;
    let extras = [
        ModMask::default(),
        numlock,
        capslock,
        scrolllock,
        numlock | capslock,
        numlock | scrolllock,
        capslock | scrolllock,
        numlock | capslock | scrolllock,
    ];
    for extra in extras {
        conn.grab_key(
            false,
            root,
            base | extra,
            keycode,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?;
    }
    Ok(())
}

fn keycode_for_keysym(conn: &RustConnection, sym: u32) -> Option<Keycode> {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    if max <= min {
        return None;
    }
    let count = max - min + 1;
    let reply = conn.get_keyboard_mapping(min, count).ok()?.reply().ok()?;
    let per = reply.keysyms_per_keycode as usize;
    if per == 0 {
        return None;
    }
    for (i, chunk) in reply.keysyms.chunks(per).enumerate() {
        if chunk.contains(&sym) {
            return Some(min + i as Keycode);
        }
    }
    None
}

/// Suppress key-repeat storms: one panel summon per 400ms.
fn debounce_ok(last_panel: &AtomicU32, time: u32) -> bool {
    let prev = last_panel.swap(time, Ordering::Relaxed);
    time.saturating_sub(prev) > 400
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
