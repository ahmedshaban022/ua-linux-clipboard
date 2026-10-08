# Rust workspace: one daemon, thin clients, GTK4/libadwaita panel

UA Clipboard is a Rust workspace — `ua-core` pure domain logic, `ua-daemon` resident watcher+storage+IPC, `ua-gtk` panel, `ua-cli` control/setup/doctor, `ua-ipc` the one shared IPC client (added 2026-10-08 to kill client triplication), `extension/` GJS companion. The daemon is the single source of truth; the panel and CLI are thin clients over the Unix-socket JSON-line protocol (the DBus `org.ua.Clipboard` mirror is a declared pending milestone riding on the same `ua_core::ipc` types).

**Why**: clipboard watching must survive the panel closing, so watching lives in a resident process (the shape clipcat validated); Rust for memory safety + performance at the "OS-feature" quality bar; GTK4/libadwaita because the product is GNOME-first (Adwaita gives the Win11-Fluent-equivalent look and automatic dark/light/accent), and it remains fully usable on KDE, wlroots, and X11.

**Considered options**: Qt6/QML via cxx-qt (rejected: heavier FFI seam, weaker GNOME look), Slint/Iced (rejected: weaker system-utility integration — portals, themes, IME), single-process app (rejected: watching dies with the panel), Electron/Tauri (rejected: weight).

**Consequences**: GTK look on KDE is "good", not native-Qt; acceptable given GNOME-first priorities. `ua-core` stays pure (no OS/UI calls); OS-specific code lives behind traits in `ua-daemon` plus the small `cfg(linux)` IPC client in `ua-ipc` (amended 2026-10-08 when the shared client moved out of the daemon).
