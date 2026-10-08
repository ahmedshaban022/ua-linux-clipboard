//! `ua-clipboard` — control the daemon, run first-run setup, diagnose.
//!
//! Subcommands: ping · list [QUERY] · pin ID · unpin ID · delete ID ·
//! clear · settings · set KEY VALUE · toggle · setup · doctor
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
  toggle            ask the daemon to toggle the panel
  setup             first-run: autostart + Super+V registration per desktop
  doctor            diagnose the setup (watching, shortcut, daemon, IPC)"
    );
}

#[cfg(target_os = "linux")]
use ua_core::ipc::{Request, Response};

#[cfg(target_os = "linux")]
mod platform {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::process::Command;

    use ua_core::ipc::{Request, Response};

    pub fn socket_path() -> String {
        std::env::var("UA_CLIPBOARD_SOCKET").unwrap_or_else(|_| {
            let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
            format!("{runtime}/ua-clipboard.sock")
        })
    }

    pub fn request(req: &Request) -> Result<Response, String> {
        let path = socket_path();
        let mut stream = UnixStream::connect(&path).map_err(|e| format!("connect {path}: {e}"))?;
        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(stream, "{json}").map_err(|e| e.to_string())?;
        stream.flush().ok();
        let mut line = String::new();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim()).map_err(|e| e.to_string())
    }

    pub fn autostart_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.config/autostart")
    }

    pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
        Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    pub fn desktop() -> String {
        std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase()
    }

    pub fn session_type() -> String {
        std::env::var("XDG_SESSION_TYPE").unwrap_or_default()
    }

    /// GNOME: register a custom keybinding via gsettings (ticket 02).
    pub fn register_gnome_shortcut(accel: &str, command: &str) -> Result<(), String> {
        let base = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ua-clipboard/";
        let sda = "org.gnome.settings-daemon.plugins.media-keys";
        let kb = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

        let existing =
            run("gsettings", &[sda, "get", "custom-keybindings"]).unwrap_or_else(|| "[]".into());
        if !existing.contains("ua-clipboard") {
            let trimmed = existing.trim_end_matches(']').trim();
            let merged = if existing.contains("@as") || trimmed == "[" {
                format!("[\"{base}\"]")
            } else {
                format!("{trimmed}, \"{base}\"]")
            };
            run("gsettings", &["set", sda, "custom-keybindings", &merged])
                .ok_or("gsettings set custom-keybindings failed")?;
        }
        run(
            "gsettings",
            &["set", kb, &format!(":{base}"), "name", "UA Clipboard"],
        )
        .ok_or("gsettings set name failed")?;
        run(
            "gsettings",
            &["set", kb, &format!(":{base}"), "command", command],
        )
        .ok_or("gsettings set command failed")?;
        run(
            "gsettings",
            &["set", kb, &format!(":{base}"), "binding", accel],
        )
        .ok_or("gsettings set binding failed")?;
        Ok(())
    }

    pub fn write_autostart() -> Result<String, String> {
        let dir = autostart_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = format!("{dir}/ua-clipboard.desktop");
        let bin = which_daemon_bin();
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=UA Clipboard\nExec={bin}\nX-GNOME-Autostart-enabled=true\nComment=Windows 11-style clipboard history\n"
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(path)
    }

    fn which_daemon_bin() -> String {
        run("which", &["ua-clipboard-daemon"]).unwrap_or_else(|| "ua-clipboard-daemon".into())
    }

    pub fn print_wlroots_snippet(accel: &str) {
        let key = accel.trim_start_matches(['<', '>']).to_lowercase();
        println!("\nAdd to your compositor config:\n");
        println!("  sway:   bindsym {key} exec ua-clipboard-panel");
        println!("  hypr:   bind = SUPER, V, exec, ua-clipboard-panel");
        println!("         (and enable the data-control/clipboard plugin for watching — see Hyprland wiki)");
        println!(
            "  (KDE Plasma 6: Settings → Shortcuts → Custom → add '{accel}' → ua-clipboard-panel)"
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
    }) {
        Ok(Response::Entries { items }) => {
            for s in items {
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

#[cfg(target_os = "linux")]
fn cmd_toggle() -> i32 {
    match request(&Request::Toggle) {
        Ok(Response::Ok) => 0,
        _ => 1,
    }
}

#[cfg(target_os = "linux")]
fn cmd_setup() -> i32 {
    println!("UA Clipboard setup\n");
    let desktop = desktop();
    let session = session_type();

    match request(&Request::GetSettings) {
        Ok(Response::Settings(s)) => {
            println!("• settings ok (shortcut {})", s.shortcut);
            if let Err(e) = register_desktop_shortcut(&s.shortcut, &desktop) {
                println!("• shortcut: FAILED ({e}) — see doctor");
            } else if desktop.contains("GNOME") {
                println!("• shortcut: registered via gsettings (GNOME)");
            }
            match write_autostart() {
                Ok(path) => println!("• autostart: {path}"),
                Err(e) => println!("• autostart: FAILED ({e})"),
            }
            if !desktop.contains("GNOME") && session.contains("wayland") {
                print_wlroots_snippet(&s.shortcut);
            }
            println!("\nNext: start the daemon (ua-clipboard-daemon) and open the panel (ua-clipboard-panel).");
            0
        }
        _ => {
            println!("daemon not running — start ua-clipboard-daemon first");
            1
        }
    }
}

#[cfg(target_os = "linux")]
fn register_desktop_shortcut(accel: &str, desktop: &str) -> Result<(), String> {
    if desktop.contains("GNOME") {
        register_gnome_shortcut(accel, "ua-clipboard-panel")
    } else {
        // KDE: GlobalShortcuts portal flow is interactive; print guidance.
        // wlroots: compositor config; snippet printed by caller.
        Ok(())
    }
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

    if desktop.contains("GNOME") && session.contains("wayland") {
        let ext_dir = format!(
            "{}/.local/share/gnome-shell/extensions/ua-clipboard@ua",
            std::env::var("HOME").unwrap_or_default()
        );
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
