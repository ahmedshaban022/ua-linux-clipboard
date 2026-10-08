# UA Clipboard

**Windows 11's Win+V clipboard experience, native on Linux.** GNOME first — KDE Plasma, sway/Hyprland and X11 too. Reliable first, beautiful second, lightweight always.

- History of text, rich text (HTML), images and links as clean preview cards
- **Pin** what matters — pins survive "Clear all" and reboots
- `Super + V` presents the panel (default; remappable)
- Picking an entry **pastes it** into the app you were in (auto-paste)
- Starts with your session; follows system dark/light; instant search
- Secrets copied from password managers are **never stored**
- English UI first, i18n/RTL-ready architecture (Arabic planned)

Design docs: [spec](.scratch/clipboard-manager-spec/spec.md) · [decision map](.scratch/clipboard-manager-spec/map.md) · [glossary](GLOSSARY.md) · [ADRs](docs/adr/)

## Status (honest)

| Piece | State |
|---|---|
| `ua-core` — domain logic (history policy, dedup/bump, caps, pin, secret filter, full-text search + highlight, shortcut parser, paste state machine) | ✅ complete, unit-tested |
| `ua-ipc` — shared JSON-line client (one implementation for daemon/CLI/panel) | ✅ done |
| `ua-daemon` — SQLite (WAL, migrations, **thumbnails**, metadata-only listing), Unix-socket IPC, admission + stats (`Status`), panic log | ✅ smoke-tested end-to-end |
| Watching — X11: **native XFixes events + native Super+V XGrabKey** (payload read via `xclip`, transitional) | ✅ CI-tested under Xvfb |
| Watching — KDE/wlroots: `wl-paste --watch` hook with **real MIME targets (secret detection works)**; native wl-clipboard-rs port is a later milestone | ✅ wired |
| Watching — GNOME Wayland (companion extension) | 🟡 experimental skeleton (text watching → daemon socket) |
| `ua-gtk` panel — single-instance, **server-side full-text search with highlighted matches**, pick, Esc, Ctrl+F | ✅ compiles; first runnable cut |
| `ua-cli` — ping/list/pin/delete/clear/settings/set/**toggle** + **setup** (starts daemon, registers per-DE) + **doctor** (backend, counters, shortcut, extension) + **uninstall [--purge]** | ✅ works |
| DBus `org.ua.Clipboard` mirror + `EntryAdded`/`HistoryChanged`/`PasteRequested` signals | ⏳ pending milestone — shared `ua_core::ipc` types make it additive |
| Auto-paste platform ports (extension inject / XTest / wtype) | ⏳ next milestone — core state machine done & tested |
| Packaging (crates.io, AUR, .deb/.rpm) | ⏳ install.sh draft; publish later |
| Non-Linux dev fallbacks (stdin IPC server, stdin watcher) | declared dev convenience, not spec surface |

Why an extension on GNOME at all? Mutter blocks the clipboard-watching protocols by design — see [ADR-0001](docs/adr/0001-gnome-companion-extension-is-load-bearing.md). That's also why other clipboard managers feel flaky there.

## Layout

```
crates/ua-core     pure domain logic (no OS, no UI) — fully unit-tested
crates/ua-ipc      the one shared IPC client (socket path + JSON-line framing)
crates/ua-daemon   resident process: watchers, SQLite, IPC, stats
crates/ua-gtk      the panel (gtk4-rs + libadwaita)
crates/ua-cli      ua-clipboard command line
extension/         GNOME Shell companion (GJS)
```

## Build & run (Linux)

```sh
# native
sudo apt install build-essential pkg-config libxcb1-dev libgtk-4-dev libadwaita-1-dev
cargo build --release

# first-run: starts the daemon, registers Super+V per desktop, autostart
./target/release/ua-clipboard setup

# play with it
./target/release/ua-clipboard ping
echo "hello" | ./target/release/ua-clipboard-daemon --capture-stdin
./target/release/ua-clipboard list
./target/release/ua-clipboard doctor
./target/release/ua-clipboard-panel   # or: ua-clipboard toggle
```

### Docker development flow (no Linux machine needed)

```sh
docker run --rm -v "$PWD:/app" -w /app rust:slim-trixie sh -c '
  apt-get update -qq && apt-get install -y -qq build-essential pkg-config \
    libxcb1-dev libgtk-4-dev libadwaita-1-dev xvfb xclip &&
  cargo test --workspace && cargo clippy --workspace -- -D warnings &&
  (Xvfb :99 & sleep 2; XDG_SESSION_TYPE=x11 DISPLAY=:99 \
   UA_CLIPBOARD_DB=/tmp/h.db UA_CLIPBOARD_SOCKET=/tmp/s.sock \
   ./target/debug/ua-clipboard-daemon & sleep 2; \
   printf canary | xclip -selection clipboard -in; sleep 2; \
   UA_CLIPBOARD_SOCKET=/tmp/s.sock ./target/debug/ua-clipboard list)'
```

## Environment notes

- **KDE Plasma 6**: watching via `ext-data-control-v1` — install `wl-clipboard ≥ 2.3`. Shortcut: Settings → Shortcuts → Custom → `ua-clipboard toggle`.
- **Hyprland**: data-control may be disabled by default; enable it (see Hyprland wiki → clipboard) — `ua-clipboard doctor` checks.
- **GNOME Wayland**: install the companion extension:
  `cp -r extension/ ~/.local/share/gnome-shell/extensions/ua-clipboard@ua/` then restart the Shell and enable it.
- **X11**: works out of the box — the daemon grabs Super+V itself (needs `xclip` for the transitional payload read).

## Roadmap to v1

1. Auto-paste ports: X11 XTest, wlroots virtual-keyboard, GNOME extension Clutter injection (spec §2 pipeline) — core state machine is done and tested in `ua-core::paste`.
2. Panel polish to the ticket-07 spec: sections, image cards, full keyboard model, undo-toast.
3. Native X11 selection read (drop xclip), image + HTML capture on Wayland.
4. DBus mirror + signals (types are shared; transport is additive).
5. Packaging (crates.io, AUR, .deb/.rpm).

MIT OR Apache-2.0.
