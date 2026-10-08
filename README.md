# UA Clipboard

**Windows 11's Win+V clipboard experience, native on Linux.** GNOME first — KDE Plasma, sway/Hyprland and X11 too. Reliable first, beautiful second, lightweight always.

- History of text, rich text (HTML), images and links as clean preview cards
- **Pin** what matters — pins survive "Clear all" and reboots
- `Super + V` opens the panel (default; remappable)
- Picking an item **pastes it** into the app you were in (auto-paste)
- Starts with your session; follows system dark/light; instant search
- Secrets copied from password managers are **never stored**
- English UI first, i18n/RTL-ready architecture (Arabic planned)

Design docs: [spec](.scratch/clipboard-manager-spec/spec.md) · [decision map](.scratch/clipboard-manager-spec/map.md) · [glossary](GLOSSARY.md) · [ADRs](docs/adr/)

## Status (honest)

| Piece | State |
|---|---|
| `ua-core` — domain logic (history policy, dedup/bump, caps, pin, secret filter, search, shortcut parser, paste state machine) | ✅ complete, **44 tests green** |
| `ua-daemon` — SQLite storage (WAL, migrations), Unix-socket JSON IPC, admission loop | ✅ compiles + **smoke-tested end-to-end** (capture, dedup, search, pin-survives-clear, persistence across restart) |
| Watching — X11 (XFixes events, native; payload via `xclip` transitional) | ✅ compiles, needs on-machine soak |
| Watching — KDE/wlroots (`wl-paste --watch` hook) | ✅ wired, needs on-machine soak |
| Watching — GNOME Wayland (companion extension) | 🟡 experimental skeleton (text watching → daemon socket) |
| `ua-gtk` panel — libadwaita window, list, search, pick | ✅ compiles; **first runnable cut — try it and react** (this is the prototype) |
| `ua-cli` — ping/list/pin/delete/clear/settings/set/toggle + `setup` + `doctor` | ✅ works (smoke-tested) |
| Auto-paste platform ports (extension inject / XTest / wtype) | ⏳ next milestone — core state machine done & tested |
| Packaging (crates.io, AUR, .deb/.rpm, install.sh) | 🟡 install.sh draft; publish later |

Why an extension on GNOME at all? Mutter blocks the clipboard-watching protocols by design — see [ADR-0001](docs/adr/0001-gnome-companion-extension-is-load-bearing.md). That's also why other clipboard managers feel flaky there.

## Layout

```
crates/ua-core     pure domain logic (no OS, no UI) — fully unit-tested
crates/ua-daemon   resident process: watchers, SQLite, IPC
crates/ua-gtk      the panel (gtk4-rs + libadwaita)
crates/ua-cli      ua-clipboard command line
extension/         GNOME Shell companion (GJS)
```

## Build & run (Linux)

```sh
# native
sudo apt install build-essential pkg-config libxcb1-dev libgtk-4-dev libadwaita-1-dev
cargo build --release

# start the daemon, then play with it
./target/release/ua-clipboard-daemon &
echo "hello" | ./target/release/ua-clipboard-daemon --capture-stdin
./target/release/ua-clipboard list
./target/release/ua-clipboard ping
./target/release/ua-clipboard doctor

# the panel
./target/release/ua-clipboard-panel

# first-run setup: autostart + Super+V (GNOME registers via gsettings)
./target/release/ua-clipboard setup
```

### Docker development flow (no Linux machine needed)

```sh
docker run --rm -v "$PWD:/app" -w /app rust:slim-trixie sh -c '
  apt-get update -qq && apt-get install -y -qq build-essential pkg-config \
    libxcb1-dev libgtk-4-dev libadwaita-1-dev &&
  cargo test --workspace && cargo clippy --workspace'
```

## Environment notes

- **KDE Plasma 6**: watching via `ext-data-control-v1` — install `wl-clipboard ≥ 2.3`.
- **Hyprland**: data-control may be disabled by default; enable it (see Hyprland wiki → clipboard) — `ua-clipboard doctor` checks.
- **GNOME Wayland**: install the companion extension:
  `cp -r extension/ ~/.local/share/gnome-shell/extensions/ua-clipboard@ua/` then restart the Shell and enable it.
- **X11**: works out of the box (needs `xclip` for the transitional payload read).

## Roadmap to v1

1. Auto-paste ports: X11 XTest, wlroots virtual-keyboard, GNOME extension Clutter injection (spec §2 pipeline) — core state machine is done and tested in `ua-core::paste`.
2. Panel polish to the ticket-07 spec: sections, image cards, keyboard model, undo-toast.
3. Native X11 selection read (drop xclip), image + HTML capture on Wayland.
4. `setup`/`doctor` hardening; packaging (crates.io, AUR, .deb/.rpm).

MIT OR Apache-2.0.
