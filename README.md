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

---

## Install

Pick one path. All of them end with `ua-clipboard setup` — that registers the shortcut and autostart for you.

### A. From source (a Rust toolchain on the machine)

```sh
sudo apt install build-essential pkg-config libxcb1-dev libgtk-4-dev libadwaita-1-dev   # Debian/Ubuntu
# Fedora: sudo dnf install gcc pkgconf-pkg-config libxcb-devel gtk4-devel libadwaita-devel
# Arch:   sudo pacman -S base-devel libxcb gtk4 libadwaita

git clone <this-repo> && cd ua-linux-clipboard
./install.sh          # builds release, installs to ~/.local/bin, runs setup
```

Make sure `~/.local/bin` is on your `PATH` (the script warns if not).

### B. No Rust toolchain? Build with Docker

Any Linux machine with Docker gets the same result — no compiler, no GTK headers installed on the host:

```sh
git clone <this-repo> && cd ua-linux-clipboard
docker run --rm -v "$PWD:/app" -w /app rust:slim-trixie sh -c '
  apt-get update -qq && apt-get install -y -qq build-essential pkg-config \
    libxcb1-dev libgtk-4-dev libadwaita-1-dev &&
  cargo build --release --workspace'
mkdir -p ~/.local/bin
cp target/release/ua-clipboard{,-daemon,-panel} ~/.local/bin/
~/.local/bin/ua-clipboard setup
```

### C. Copying prebuilt binaries from another machine

The binaries in `target/release/` are dynamically linked against glibc/GTK4 — copy `ua-clipboard`, `ua-clipboard-daemon`, `ua-clipboard-panel` to `~/.local/bin/` on a machine from the **same distro family** (or one with equal-or-newer glibc and GTK 4.x + libadwaita installed), then run `ua-clipboard setup` there.

### D. Package managers

crates.io (`cargo install` / `cargo-binstall`), AUR, COPR and .deb/.rpm assets are **coming** — see the status table below. Until then, A–C are the supported paths.

### GNOME Wayland extra step (required)

Mutter blocks clipboard-watching protocols by design ([ADR-0001](docs/adr/0001-gnome-companion-extension-is-load-bearing.md)) — on GNOME Wayland the companion extension is the watcher:

```sh
cp -r extension/ ~/.local/share/gnome-shell/extensions/ua-clipboard@ua/
```

Then restart GNOME Shell (`Alt+F2` → `r` on X11, or log out/in on Wayland) and enable **UA Clipboard** in the Extension Manager. `ua-clipboard doctor` verifies it.

### Uninstall

```sh
ua-clipboard uninstall           # reverses shortcut + autostart
ua-clipboard uninstall --purge   # also deletes history and settings
```

---

## User guide

### Daily use

1. Copy anything (Ctrl+C) — it appears in history within a moment.
2. Press **Super+V** anywhere — the panel opens (on X11 the daemon owns the grab; on GNOME it's registered by `setup`; sway/Hyprland need one bind line, printed by `setup`).
3. Type to **search** (full-text, case-insensitive, matches highlighted), or arrow through entries.
4. **Enter** or click picks the entry → the panel closes and the entry is pasted into the app you were in.
5. **Esc** closes without pasting.

| Key | Action |
|---|---|
| `↑` / `↓` | move through entries |
| `Enter` | pick → paste |
| `Esc` | close the panel |
| `Ctrl+F` | jump to the search field |
| type | search as you type |

Pinning and deleting from the panel's hover buttons/context menu land with the panel milestone — until then use the CLI below.

### The CLI

```
ua-clipboard ping                 # is the daemon alive?
ua-clipboard list                 # show history (newest first)
ua-clipboard list <query>         # filtered, e.g: list http
ua-clipboard pin 3                # pin entry 3   (pins never expire)
ua-clipboard unpin 3
ua-clipboard delete 3             # delete one entry
ua-clipboard clear                # delete ALL unpinned entries (pins kept)
ua-clipboard toggle               # present the panel (what Super+V runs)
ua-clipboard settings             # show current settings
ua-clipboard set shortcut "<Ctrl><Alt>v"   # remap the shortcut
ua-clipboard set max_entries 200
ua-clipboard set max_image_mb 32
ua-clipboard set autostart false
ua-clipboard setup                # (re)run first-run registration
ua-clipboard doctor               # diagnose everything
```

Defaults: 100 entries, images up to 16 MB, autostart on — all changeable. Shortcut strings use GTK syntax: `<Super>v`, `<Ctrl><Shift>t`, `<Alt>F4`.

### Password managers

KeePassXC (and apps following the same convention) mark clipboard content as sensitive. UA Clipboard **never stores** those copies — `doctor` shows a running counter of skipped secrets so you can verify it works.

### Where your data lives

| Path | What |
|---|---|
| `~/.local/state/ua-clipboard/history.db` | history + settings (SQLite, WAL) |
| `~/.local/state/ua-clipboard/daemon.log` | panic log |
| `$XDG_RUNTIME_DIR/ua-clipboard.sock` | IPC socket (per session) |
| `~/.config/autostart/ua-clipboard.desktop` | autostart entry (written by `setup`) |

Back up `history.db` to carry your history (including pins) to another machine.

### Troubleshooting

Always start with `ua-clipboard doctor` — it checks the daemon, watcher backend, capture/secret counters, shortcut registration (GNOME), the companion extension, and autostart.

- **Nothing appears when I copy** — run `doctor`. On KDE/sway: is `wl-paste` installed (`wl-clipboard ≥ 2.3`)? On Hyprland: enable the data-control/clipboard plugin, then restart the daemon. On GNOME Wayland: is the extension installed and enabled?
- **Super+V does nothing** — GNOME: re-run `setup` (it registers via gsettings) and check `doctor`. KDE: add the custom shortcut manually (Settings → Shortcuts → `ua-clipboard toggle`). sway/Hyprland: add the bind line `setup` prints. X11: the daemon grabs it — is the daemon running?
- **The daemon isn't running** — `ua-clipboard-daemon &` (or re-run `setup`, which starts it), or log out/in if autostart is enabled.

---

## Status (honest)

| Piece | State |
|---|---|
| `ua-core` — domain logic (history policy, dedup/bump, caps, pin, secret filter, full-text search + highlight, shortcut parser, paste state machine) | ✅ complete, unit-tested |
| `ua-ipc` — shared JSON-line client (one implementation for daemon/CLI/panel) | ✅ done |
| `ua-daemon` — SQLite (WAL, migrations, **thumbnails**, blob-free listing/search), Unix-socket IPC, admission + stats (`Status`), panic log | ✅ smoke-tested end-to-end |
| Watching — X11: **native XFixes events + native Super+V XGrabKey** (payload read via `xclip`, transitional) | ✅ CI-tested under Xvfb |
| Watching — KDE/wlroots: `wl-paste --watch` hook with **real MIME targets (secret detection works)**; native wl-clipboard-rs port is a later milestone | ✅ wired |
| Watching — GNOME Wayland (companion extension) | 🟡 experimental skeleton (text watching → daemon socket) |
| `ua-gtk` panel — single-instance, **server-side full-text search with highlighted matches**, pick, Esc, Ctrl+F | ✅ compiles; first runnable cut |
| `ua-cli` — ping/list/pin/delete/clear/settings/set/**toggle** + **setup** + **doctor** + **uninstall [--purge]** | ✅ works |
| **Auto-paste platform ports** (extension inject / XTest / wtype) | ⏳ next milestone — the pick currently sets the entry as most-recent and is acknowledged; Ctrl+V injection wiring is the remaining piece. Core state machine done & tested. |
| DBus `org.ua.Clipboard` mirror + signals | ⏳ pending milestone — shared `ua_core::ipc` types make it additive |
| Packaging (crates.io, AUR, .deb/.rpm) | ⏳ install.sh + paths A–C work; stores coming |
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

## Development

```sh
# native
sudo apt install build-essential pkg-config libxcb1-dev libgtk-4-dev libadwaita-1-dev
cargo test --workspace && cargo clippy --workspace -- -D warnings

# Docker (no Linux toolchain needed)
docker run --rm -v "$PWD:/app" -w /app rust:slim-trixie sh -c '
  apt-get update -qq && apt-get install -y -qq build-essential pkg-config \
    libxcb1-dev libgtk-4-dev libadwaita-1-dev xvfb xclip &&
  cargo test --workspace && cargo clippy --workspace -- -D warnings'
```

CI (`.github/workflows/`) runs fmt, clippy, tests, a release build, and an X11 integration test under Xvfb on every push.

## Roadmap to v1

1. Auto-paste ports: X11 XTest, wlroots virtual-keyboard, GNOME extension Clutter injection (spec §2 pipeline).
2. Panel polish to the ticket-07 spec: sections, image cards, full keyboard model, undo-toast.
3. Native X11 selection read (drop xclip), image + HTML capture on Wayland.
4. DBus mirror + signals (types are shared; transport is additive).
5. Packaging (crates.io, AUR, .deb/.rpm).

MIT OR Apache-2.0.
