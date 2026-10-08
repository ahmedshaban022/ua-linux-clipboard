# Rust workspace: one daemon, thin clients, GTK4/libadwaita panel

UA Clipboard is a five-crate Rust workspace (`ua-core` pure domain logic, `ua-daemon` resident watcher+storage+DBus, `ua-gtk` panel, `ua-cli` control/setup/doctor, `extension/` GJS companion). The daemon is the single source of truth; the panel and CLI are thin clients over DBus `org.ua.Clipboard`. UI is gtk4-rs + libadwaita, vanilla (no relm4).

**Why**: clipboard watching must survive the panel closing, so watching lives in a resident process (the shape clipcat validated); Rust for memory safety + performance at the "OS-feature" quality bar; GTK4/libadwaita because the product is GNOME-first (Adwaita gives the Win11-Fluent-equivalent look and automatic dark/light/accent), and it remains fully usable on KDE, wlroots, and X11.

**Considered options**: Qt6/QML via cxx-qt (rejected: heavier FFI seam, weaker GNOME look), Slint/Iced (rejected: weaker system-utility integration — portals, themes, IME), single-process app (rejected: watching dies with the panel), Electron/Tauri (rejected: weight).

**Consequences**: GTK look on KDE is "good", not native-Qt; acceptable given GNOME-first priorities. All OS-specific code sits behind traits in `ua-daemon` so `ua-core` stays pure and fully unit-testable on any host.
