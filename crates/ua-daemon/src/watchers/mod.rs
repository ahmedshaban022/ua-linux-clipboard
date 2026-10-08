//! Clipboard watching per environment (spec §1, ticket 01).
//!
//! - GNOME Wayland: the Companion Extension pushes `Capture` requests over
//!   the IPC socket (Mutter blocks the data-control protocols — ADR-0001).
//!   Nothing to spawn here; we just log and wait.
//! - KDE / wlroots Wayland: exec `wl-paste --watch <self> --capture-stdin`.
//!   wl-clipboard ≥ 2.3 speaks ext-data-control on KWin; wlr-data-control
//!   on sway/Hyprland (Hyprland users must enable it — `doctor` checks).
//!   This shells out to wl-clipboard deliberately (README status): the
//!   crate's watch API has churned and the cliphist-shaped pipeline is
//!   battle-tested; the native wl-clipboard-rs port is a later milestone.
//! - X11: x11rb + XFixes selection events (below, `x11`), which also owns
//!   the Super+V XGrabKey so the shortcut works with no setup on X11.
//! - Unknown/dev: stdin lines become text captures.

use std::sync::mpsc::Sender;

use ua_core::clock::Clock as _;
use ua_core::ipc::CapturePayload;
use ua_core::model::RawCapture;

/// Present the (single-instance) panel. Shared by the daemon's Toggle
/// handler and the X11 shortcut grab — one definition, one place.
pub fn spawn_panel() {
    use std::process::{Command, Stdio};
    let _ = Command::new("ua-clipboard-panel")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// GNOME Wayland — Companion Extension pushes captures over IPC.
    Extension,
    /// KDE Plasma 6 / wlroots — wl-clipboard data-control.
    WlPaste,
    /// X11 — XFixes selection events + XGrabKey shortcut.
    X11,
    /// Development: captures arrive on stdin, one per line.
    Stdin,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Extension => "gnome-extension",
            Backend::WlPaste => "wl-paste",
            Backend::X11 => "x11",
            Backend::Stdin => "stdin(dev)",
        }
    }
}

pub fn detect() -> Backend {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();
    let wayland = std::env::var("WAYLAND_DISPLAY").is_ok() || session == "wayland";
    if wayland {
        if desktop.contains("GNOME") {
            Backend::Extension
        } else {
            Backend::WlPaste
        }
    } else if session == "x11" || std::env::var("DISPLAY").is_ok() {
        Backend::X11
    } else {
        Backend::Stdin
    }
}

/// `shortcut` is the settings' accelerator string; the X11 backend uses it
/// for its XGrabKey (other backends ignore it).
pub fn spawn(tx: Sender<RawCapture>, shortcut: &str) {
    let shortcut = shortcut.to_string();
    let backend = detect();
    println!("[daemon] clipboard backend: {}", backend.label());
    match backend {
        Backend::Extension => {
            println!(
                "[daemon] GNOME Wayland: waiting for the Companion Extension \
                 (see extension/ — captures arrive as Capture requests over the socket)"
            );
        }
        Backend::WlPaste => {
            std::thread::spawn(wayland_watch);
        }
        Backend::X11 => {
            #[cfg(target_os = "linux")]
            std::thread::spawn(move || x11::watch(tx, &shortcut));
            #[cfg(not(target_os = "linux"))]
            let _ = (tx, shortcut);
        }
        Backend::Stdin => {
            std::thread::spawn(move || stdin_watch(tx));
        }
    }
}

fn stdin_watch(tx: Sender<RawCapture>) {
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = line.trim_end_matches(['\r', '\n']).to_string();
                if text.is_empty() {
                    continue;
                }
                let at = ua_core::clock::SystemClock.now_ms();
                let _ = tx.send(RawCapture::text(at, text));
            }
        }
    }
}

/// KDE/wlroots: `wl-paste --watch` runs a command per clipboard change with
/// the new content on stdin — we exec ourselves in capture mode, which
/// pushes a `Capture` request to our own socket. This is the proven
/// cliphist-shaped pipeline; it survives wl-clipboard crate API churn.
fn wayland_watch() {
    use std::process::{Command, Stdio};

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[daemon] cannot resolve own path for wl-paste hook: {e}");
            return;
        }
    };
    let status = Command::new("wl-paste")
        .arg("--watch")
        .arg(exe)
        .arg("--capture-stdin")
        .stdin(Stdio::inherit())
        .status();
    match status {
        Ok(s) => eprintln!("[daemon] wl-paste --watch exited: {s} (is wl-clipboard installed?)"),
        Err(e) => eprintln!(
            "[daemon] wl-paste not available: {e}. Install wl-clipboard ≥ 2.3 \
             (and on Hyprland enable data-control in the compositor config)."
        ),
    }
}

/// Entry point for `ua-clipboard-daemon --capture-stdin`: read one capture
/// from stdin and push it to the daemon over IPC. Used as the wl-paste
/// --watch command (see `wayland_watch`).
pub fn capture_stdin_main() -> i32 {
    use std::io::Read;

    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        return 1;
    }
    let text = text.trim_end_matches(['\r', '\n', '\0']).to_string();
    if text.is_empty() {
        return 0;
    }
    // We run synchronously inside the watch callback, so the selection is
    // still current: fetch its MIME targets so secret detection (spec §5)
    // works on KDE/sway/Hyprland too.
    let offers = list_targets();
    let req = ua_core::ipc::Request::Capture {
        capture: CapturePayload {
            offers,
            text: Some(text),
            html: None,
            image_b64: None,
            uris: None,
            source_app: None,
        },
    };
    match ua_ipc::request(&req) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("capture-stdin: {e}");
            1
        }
    }
}

fn list_targets() -> Vec<String> {
    use std::process::Command;

    Command::new("wl-paste")
        .args(["--list-targets"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub mod x11;
