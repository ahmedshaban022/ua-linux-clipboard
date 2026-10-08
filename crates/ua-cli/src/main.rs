//! `ua-clipboard` — control the daemon, run first-run setup, diagnose.
//!
//! Subcommands: ping · list [QUERY] · pin ID · unpin ID · delete ID ·
//! clear · settings · set KEY VALUE · toggle · setup · doctor · uninstall
//!
//! Non-Linux hosts: the CLI explains it needs Linux (dev convenience only).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[1..];

    let code = match cmd {
        "ping" => cmd_ping(),
        "list" => cmd_list(rest.first().map(String::as_str).unwrap_or("")),
        "pin" | "unpin" => cmd_pin(rest.first().and_then(|s| s.parse().ok()), cmd == "pin"),
        "delete" => cmd_delete(rest.first().and_then(|s| s.parse().ok())),
        "clear" => cmd_clear(),
        "settings" => cmd_settings(),
        "set" => cmd_set(rest),
        "toggle" => cmd_toggle(),
        "setup" => cmd_setup(),
        "doctor" => cmd_doctor(),
        "uninstall" => cmd_uninstall(rest.iter().any(|a| a == "--purge")),
        _ => {
            print_help();
            0
        }
    };
    std::process::exit(code);
}

fn print_help() {
    println!(
        "ua-clipboard — Windows 11-style clipboard history for Linux

Usage: ua-clipboard <COMMAND>

  ping              check the daemon is alive
  list [QUERY]      show history (optionally filtered)
  pin ID            pin an entry        unpin ID   unpin it
  delete ID         delete an entry     clear      delete all unpinned
  settings          show settings       set K V    change one (shortcut,
                                                    max_entries, max_image_mb, autostart)
  toggle            present the panel (what Super+V runs)
  setup             first-run: daemon, autostart + Super+V per desktop
  doctor            diagnose the setup (watching, shortcut, daemon, IPC)
  uninstall [--purge]  remove registrations; --purge also deletes history"
    );
}

#[cfg(target_os = "linux")]
use ua_core::ipc::{Request, Response};

#[cfg(target_os = "linux")]
mod platform {
    use std::process::Command;

    pub use ua_ipc::request;
    pub use ua_ipc::socket_path;

    pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
        Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    pub fn spawn_detached(cmd: &str, args: &[&str]) -> bool {
        Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }

    pub fn desktop() -> String {
        std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase()
    }

    pub fn session_type() -> String {
        std::env::var("XDG_SESSION_TYPE").unwrap_or_default()
    }

    pub fn autostart_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.config/autostart")
    }

    pub fn state_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.local/state/ua-clipboard")
    }

    /// GNOME settings-daemon schemas (custom keybinding registration).
    const GSDA: &str = "org.gnome.settings-daemon.plugins.media-keys";
    const GSDA_KB: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

    /// GNOME: register a custom keybinding via gsettings (ticket 02).
    pub fn register_gnome_shortcut(accel: &str, command: &str) -> Result<(), String> {
        let base = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ua-clipboard/";

        let existing =
            run("gsettings", &[GSDA, "get", "custom-keybindings"]).unwrap_or_else(|| "[]".into());
        if !existing.contains("ua-clipboard") {
            let trimmed = existing.trim_end_matches(']').trim();
            let merged = if existing.contains("@as") || trimmed == "[" {
                format!("[\"{base}\"]")
            } else {
                format!("{trimmed}, \"{base}\"]")
            };
            run("gsettings", &["set", GSDA, "custom-keybindings", &merged])
                .ok_or("gsettings set custom-keybindings failed")?;
        }
        run(
            "gsettings",
            &["set", GSDA_KB, &format!(":{base}"), "name", "UA Clipboard"],
        )
        .ok_or("gsettings set name failed")?;
        run(
            "gsettings",
            &["set", GSDA_KB, &format!(":{base}"), "command", command],
        )
        .ok_or("gsettings set command failed")?;
        run(
            "gsettings",
            &["set", GSDA_KB, &format!(":{base}"), "binding", accel],
        )
        .ok_or("gsettings set binding failed")?;
        Ok(())
    }

    /// Unregister our GNOME keybinding entry if present.
    pub fn unregister_gnome_shortcut() -> Result<(), String> {
        let existing =
            run("gsettings", &[GSDA, "get", "custom-keybindings"]).unwrap_or_else(|| "[]".into());
        if !existing.contains("ua-clipboard") {
            return Ok(());
        }
        // Parse the printed array crudely: keep every quoted path but ours.
        let kept: Vec<&str> = existing
            .split(['[', ']', ','])
            .map(|p| p.trim().trim_matches(['\'', '"']))
            .filter(|p| p.starts_with('/') && !p.contains("ua-clipboard"))
            .collect();
        let merged = if kept.is_empty() {
            "[]".to_string()
        } else {
            let inner: Vec<String> = kept.iter().map(|p| format!("'{p}'")).collect();
            format!("[{}]", inner.join(", "))
        };
        run("gsettings", &["set", GSDA, "custom-keybindings", &merged])
            .ok_or("gsettings set custom-keybindings failed")?;
        Ok(())
    }

    pub fn gnome_shortcut_registered() -> bool {
        run("gsettings", &["get", GSDA, "custom-keybindings"])
            .map(|v| v.contains("ua-clipboard"))
            .unwrap_or(false)
    }

    pub fn write_autostart() -> Result<String, String> {
        let dir = autostart_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = format!("{dir}/ua-clipboard.desktop");
        let bin =
            run("which", &["ua-clipboard-daemon"]).unwrap_or_else(|| "ua-clipboard-daemon".into());
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=UA Clipboard\nExec={bin}\nX-GNOME-Autostart-enabled=true\nComment=Windows 11-style clipboard history\n"
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn print_wlroots_snippet(accel: &str) {
        let key = accel.trim_start_matches(['<', '>']).to_lowercase();
        println!("\nAdd to your compositor config:\n");
        println!("  sway:   bindsym {key} exec ua-clipboard toggle");
        println!("  hypr:   bind = SUPER, V, exec, ua-clipboard toggle");
        println!("         (and enable the data-control/clipboard plugin for watching — see Hyprland wiki)");
        println!(
            "  (KDE Plasma 6: Settings → Shortcuts → Custom → add '{accel}' → ua-clipboard toggle)"
        );
    }
}

#[cfg(target_os = "linux")]
use platform::*;

// ---- command implementations (Linux) ----

#[cfg(target_os = "linux")]
fn cmd_ping() -> i32 {
    match request(&Request::Ping) {
        Ok(Response::Pong) => {
            println!("daemon alive on {}", socket_path());
            0
        }
        Ok(Response::Error(e)) => {
            println!("daemon error: {e}");
            1
        }
        _ => {
            println!("no daemon at {}", socket_path());
            1
        }
    }
}

#[cfg(target_os = "linux")]
fn cmd_list(query: &str) -> i32 {
    match request(&Request::List {
        query: query.to_string(),
        offset: 0,
    }) {
        Ok(Response::Entries { entries }) => {
            for s in entries {
                let pin = if s.pinned { "[pin] " } else { "" };
                println!("{:>4}  {pin}{}", s.id, s.preview);
            }
            0
        }
        Ok(Response::Error(e)) => {
            println!("error: {e}");
            1
        }
        _ => {
            println!("unexpected response (is the daemon an older version?)");
            1
        }
    }
}

#[cfg(target_os = "linux")]
fn cmd_pin(id: Option<u64>, pin: bool) -> i32 {
    match id {
        Some(id) => match request(&Request::Pin { id, pinned: pin }) {
            Ok(Response::Ok) => 0,
            Ok(Response::Error(e)) => {
                println!("error: {e}");
                1
            }
            _ => 1,
        },
        None => {
            println!(
                "usage: ua-clipboard {} ID",
                if pin { "pin" } else { "unpin" }
            );
            2
        }
    }
}

#[cfg(target_os = "linux")]
fn cmd_delete(id: Option<u64>) -> i32 {
    match id {
        Some(id) => match request(&Request::Delete { id }) {
            Ok(Response::Ok) => 0,
            Ok(Response::Error(e)) => {
                println!("error: {e}");
                1
            }
            _ => 1,
        },
        None => {
            println!("usage: ua-clipboard delete ID");
            2
        }
    }
}

#[cfg(target_os = "linux")]
fn cmd_clear() -> i32 {
    match request(&Request::ClearAll) {
        Ok(Response::Ok) => {
            println!("cleared (pins kept)");
            0
        }
        _ => 1,
    }
}

#[cfg(target_os = "linux")]
fn cmd_settings() -> i32 {
    match request(&Request::GetSettings) {
        Ok(Response::Settings(s)) => {
            println!("shortcut     = {}", s.shortcut);
            println!("max_entries  = {}", s.max_entries);
            println!("max_image_mb = {}", s.max_image_mb);
            println!("autostart    = {}", s.autostart);
            0
        }
        _ => 1,
    }
}

#[cfg(target_os = "linux")]
fn cmd_set(rest: &[String]) -> i32 {
    if rest.len() != 2 {
        println!("usage: ua-clipboard set KEY VALUE   (shortcut | max_entries | max_image_mb | autostart)");
        return 2;
    }
    let current = match request(&Request::GetSettings) {
        Ok(Response::Settings(s)) => s,
        _ => {
            println!("cannot read current settings");
            return 1;
        }
    };
    let mut s = current.clone();
    let (key, val) = (&rest[0], &rest[1]);
    match key.as_str() {
        "shortcut" => s.shortcut = val.clone(),
        "max_entries" => match val.parse() {
            Ok(v) => s.max_entries = v,
            Err(_) => {
                println!("max_entries must be a number");
                return 2;
            }
        },
        "max_image_mb" => match val.parse() {
            Ok(v) => s.max_image_mb = v,
            Err(_) => {
                println!("max_image_mb must be a number");
                return 2;
            }
        },
        "autostart" => s.autostart = val == "true" || val == "1",
        _ => {
            println!("unknown setting: {key}");
            return 2;
        }
    }
    match request(&Request::SetSettings { settings: s }) {
        Ok(Response::Ok) => {
            println!("saved");
            0
        }
        Ok(Response::Error(e)) => {
            println!("error: {e}");
            1
        }
        _ => 1,
    }
}

/// Spec §1: this is what Super+V runs. The daemon presents the
/// single-instance panel; without a daemon we present it directly.
#[cfg(target_os = "linux")]
fn cmd_toggle() -> i32 {
    match request(&Request::Toggle) {
        Ok(Response::Ok) => 0,
        _ if spawn_detached("ua-clipboard-panel", &[]) => 0,
        _ => {
            println!("cannot present the panel (no daemon, no ua-clipboard-panel)");
            1
        }
    }
}

/// Spec §7 flow: detect DE → register Super+V → autostart → Hyprland
/// data-control check → extension guidance (GNOME) → daemon running.
#[cfg(target_os = "linux")]
fn cmd_setup() -> i32 {
    println!("UA Clipboard setup\n");
    let desktop = desktop();
    let session = session_type();

    // 1. Daemon up first so registration results are visible immediately.
    if !matches!(request(&Request::Ping), Ok(Response::Pong)) {
        if spawn_detached("ua-clipboard-daemon", &[]) {
            println!("• daemon: started");
            std::thread::sleep(std::time::Duration::from_millis(500));
        } else {
            println!("• daemon: could not start ua-clipboard-daemon — is it installed?");
            return 1;
        }
    } else {
        println!("• daemon: already running");
    }

    let shortcut = match request(&Request::GetSettings) {
        Ok(Response::Settings(s)) => s.shortcut,
        _ => "<Super>v".to_string(),
    };

    // 2. Shortcut per DE.
    if desktop.contains("GNOME") {
        match register_gnome_shortcut(&shortcut, "ua-clipboard toggle") {
            Ok(()) => println!("• shortcut: {shortcut} registered via gsettings (GNOME)"),
            Err(e) => println!("• shortcut: FAILED ({e}) — see doctor"),
        }
    } else if session.contains("wayland") {
        // wlroots/KDE: portal or compositor config — print exact steps.
        println!(
            "• shortcut: register in your compositor/desktop settings → 'ua-clipboard toggle'"
        );
        print_wlroots_snippet(&shortcut);
    } else {
        // X11: the daemon's XGrabKey owns the shortcut — nothing to write.
        println!("• shortcut: {shortcut} grabbed by the daemon (X11)");
    }

    // 3. Autostart.
    match write_autostart() {
        Ok(path) => println!("• autostart: {path}"),
        Err(e) => println!("• autostart: FAILED ({e})"),
    }

    // 4. Hyprland data-control check (watching depends on it).
    if desktop.contains("HYPRLAND") && session.contains("wayland") {
        println!("• Hyprland: ensure the data-control/clipboard plugin is enabled, then restart the daemon");
    }

    // 5. Companion Extension guidance (GNOME Wayland — ADR-0001).
    if desktop.contains("GNOME") && session.contains("wayland") {
        let home = std::env::var("HOME").unwrap_or_default();
        let ext = format!("{home}/.local/share/gnome-shell/extensions/ua-clipboard@ua");
        if std::path::Path::new(&ext).exists() {
            println!("• companion extension: found");
        } else {
            println!("• companion extension: install with");
            println!("    cp -r extension/ {ext}/");
            println!("  then restart GNOME Shell and enable it (doctor verifies)");
        }
    }

    println!("\nSetup complete — press {shortcut} or run: ua-clipboard toggle");
    0
}

#[cfg(target_os = "linux")]
fn cmd_doctor() -> i32 {
    println!("UA Clipboard doctor\n");
    let mut bad = 0;

    let desktop = desktop();
    let session = session_type();
    println!("• desktop: {desktop} (session: {session})");

    match request(&Request::Ping) {
        Ok(Response::Pong) => println!("• daemon: alive ({})", socket_path()),
        _ => {
            println!("• daemon: NOT REACHABLE — start ua-clipboard-daemon");
            bad += 1;
        }
    }

    // Watcher + counters from the daemon itself.
    match request(&Request::Status) {
        Ok(Response::Status {
            backend,
            captures,
            secrets_skipped,
            oversize_skipped,
        }) => {
            println!("• watcher backend: {backend}");
            println!(
                "• stats (since daemon start): {captures} captures, {secrets_skipped} secrets skipped, {oversize_skipped} oversize images skipped"
            );
            if backend == "gnome-extension" {
                println!("  (GNOME: captures appear only with the Companion Extension installed)");
            }
        }
        _ => println!("• status: unavailable (older daemon?)"),
    }

    if session.contains("wayland") && !desktop.contains("GNOME") {
        match run("which", &["wl-paste"]) {
            Some(p) => println!("• wl-paste: {p}"),
            None => {
                println!("• wl-paste: MISSING — install wl-clipboard ≥ 2.3");
                bad += 1;
            }
        }
        if desktop.contains("HYPRLAND") {
            println!("• Hyprland: ensure the data-control/clipboard plugin is enabled");
        }
    }

    // Shortcut registration per DE.
    if desktop.contains("GNOME") {
        if gnome_shortcut_registered() {
            println!("• shortcut: registered (gsettings)");
        } else {
            println!("• shortcut: NOT registered — run ua-clipboard setup");
            bad += 1;
        }
    }

    if desktop.contains("GNOME") && session.contains("wayland") {
        let home = std::env::var("HOME").unwrap_or_default();
        let ext_dir = format!("{home}/.local/share/gnome-shell/extensions/ua-clipboard@ua");
        if std::path::Path::new(&ext_dir).exists() {
            println!("• companion extension: found");
        } else {
            println!("• companion extension: NOT INSTALLED — copy extension/ to {ext_dir}");
            bad += 1;
        }
    }

    if std::path::Path::new(&format!("{}/ua-clipboard.desktop", autostart_dir())).exists() {
        println!("• autostart: present");
    } else {
        println!("• autostart: absent (run ua-clipboard setup)");
    }

    if bad == 0 {
        println!("\nAll checks passed.");
        0
    } else {
        println!("\n{bad} issue(s) found.");
        1
    }
}

/// Spec §7: uninstall reverses registrations; --purge also removes data.
#[cfg(target_os = "linux")]
fn cmd_uninstall(purge: bool) -> i32 {
    println!("UA Clipboard uninstall\n");
    let desktop = desktop();
    let mut bad = 0;

    if desktop.contains("GNOME") {
        match unregister_gnome_shortcut() {
            Ok(()) => println!("• shortcut: unregistered"),
            Err(e) => {
                println!("• shortcut: FAILED ({e})");
                bad += 1;
            }
        }
    } else {
        println!("• shortcut: remove the bind you added for 'ua-clipboard toggle'");
    }

    let autostart = format!("{}/ua-clipboard.desktop", autostart_dir());
    match std::fs::remove_file(&autostart) {
        Ok(()) => println!("• autostart: removed ({autostart})"),
        Err(_) => println!("• autostart: not present"),
    }

    let socket = socket_path();
    let _ = std::fs::remove_file(&socket);
    println!("• socket: cleaned ({socket})");

    if purge {
        let state = state_dir();
        match std::fs::remove_dir_all(&state) {
            Ok(()) => println!("• history + settings: DELETED ({state})"),
            Err(_) => println!("• history: nothing to delete ({state})"),
        }
    } else {
        println!("• history kept (use --purge to delete {})", state_dir());
    }

    println!("• reminder: stop the running daemon (pkill -f ua-clipboard-daemon)");
    println!(
        "• reminder: remove ~/.local/share/gnome-shell/extensions/ua-clipboard@ua if installed"
    );

    if bad == 0 {
        0
    } else {
        1
    }
}

// ---- non-Linux stubs ----

#[cfg(not(target_os = "linux"))]
fn cmd_ping() -> i32 {
    println!("ua-clipboard requires Linux.");
    1
}
#[cfg(not(target_os = "linux"))]
fn cmd_list(_: &str) -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_pin(_: Option<u64>, _: bool) -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_delete(_: Option<u64>) -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_clear() -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_settings() -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_set(_: &[String]) -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_toggle() -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_setup() -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_doctor() -> i32 {
    cmd_ping()
}
#[cfg(not(target_os = "linux"))]
fn cmd_uninstall(_: bool) -> i32 {
    cmd_ping()
}
