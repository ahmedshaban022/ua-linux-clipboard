# UA Clipboard — v1 Specification

- **Date**: 2026-10-08 · **State**: all wayfinder decisions resolved ([map](map.md)) · **Product**: UA Clipboard
- **Mission**: the Windows 11 Win+V clipboard experience, native on Linux — reliable first, beautiful second, lightweight always.
- **Priorities**: (1) Reliability — never misses a copy, paste always lands; (2) Win11-grade UI; (3) low footprint (idle daemon < 30MB RSS target, panel opens < 150ms).

## 1. Supported environments

| Environment | Watching | Super+V registration | Auto-paste |
|---|---|---|---|
| GNOME Wayland | **Companion Extension** → DBus → daemon (Mutter blocks data-control protocols) | gsettings custom-keybindings → `ua-clipboard toggle` | Extension: StClipboard set + Clutter VirtualInputDevice Ctrl+V |
| GNOME X11 | x11rb XFixes | same gsettings path | x11rb XTest |
| KDE Plasma 6 (Wayland/X11) | wl-clipboard-rs `ext-data-control-v1` (Wayland) / XFixes (X11) | GlobalShortcuts portal | KWin virtual-keyboard / XTest; Xwayland fallback |
| sway / Hyprland (wlroots) | wl-clipboard-rs `wlr-data-control` (Hyprland: user must enable data-control; `doctor` detects) | compositor config bind (`setup` emits snippet) | `zwp_virtual_keyboard_manager_v1` |
| Any X11 | x11rb XFixes | XGrabKey (NumLock/ScrollLock masked) | x11rb XTest |

**Degrade rule**: without the GNOME extension the app still works — centered panel, manual Ctrl+V — and tells the user what they're missing. Never break, always degrade.

## 2. Architecture

Five-crate Rust workspace; the daemon is the single source of truth, panel and CLI are thin clients.

```
ua-core      pure domain: Entry model, history policy, pins, search, secret-filter, settings.
ua-daemon    resident process: ClipboardWatcher impls, SQLite, DBus org.ua.Clipboard + signals.
ua-gtk       the panel: gtk4-rs + libadwaita; daemon client; triggers paste pipeline.
ua-cli       `ua-clipboard toggle|list|pin|delete|setup|doctor`.
extension/   GJS GNOME Shell companion: watch, flyout anchoring, paste injection.
```

**IPC (DBus `org.ua.Clipboard`)**: methods `List(offset,query) → [Entry]`, `Get(id)`, `Select(id)` (set system clipboard), `Pin(id,bool)`, `Delete(id)`, `ClearAll()` (keeps pins), `Settings()`, `SetSetting(key,val)`; signals `EntryAdded`, `HistoryChanged`, `PasteRequested(id)`. Unix-socket mirror for headless/testing.

**Paste pipeline** (runs after pick, panel already closed): wait focus-return (≤300ms) → set selection → wait ownership confirm (≤200ms) → inject Ctrl+V → retry ≤2 → on failure show fallback toast "Clipboard set — press Ctrl+V". This state machine is the anti-"sometimes works sometimes not" core; it is unit-tested with a fake clock.

## 3. Domain model

See [GLOSSARY.md](../../../GLOSSARY.md). **Entry** kinds: `Text`, `RichText` (HTML), `Image` (PNG), `Uris`. Capture offers `text/plain;charset=utf-8`, `text/html`, `image/png`, `text/uri-list`.

## 4. Storage (SQLite, rusqlite bundled, WAL)

```sql
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);           -- schema_version, settings
CREATE TABLE entries(
  id INTEGER PRIMARY KEY, kind TEXT NOT NULL, text TEXT, html TEXT,
  image_blob BLOB, thumb_blob BLOB, source_app TEXT,
  copied_at INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0,
  pin_order INTEGER, preview TEXT NOT NULL);
```

Migrations: forward-only, `schema_version` in `meta`. Caps (defaults, configurable): 100 entries, image ≤ 16MB (oversize → text entries still stored with a note; images skipped+logged), thumbnails ≤ 96dp generated on insert. Pins never evicted.

## 5. History policy

- Consecutive duplicates collapse (re-copy bumps the existing entry to top, refreshes timestamp — Win11 behavior).
- Equal-content re-copy after intervening copies: move to top, don't duplicate.
- **Secrets never stored**: entries offering `x-kde-passwordManagerHint` (or `application/x-authenticator`… confirmed at implementation) are dropped and counted in `doctor`.
- Clear-all keeps pins. Unpinned history clears on explicit user action only (no auto-wipe on reboot — we persist across sessions like Windows pins do, for all entries; this beats Windows and is a feature).

## 6. Panel UX (ticket 07, normative)

420×560dp rounded flyout; search field on top (instant case-insensitive substring, highlighted); **Pinned** section then **History** (newest first, relative time, source app). Cards: text 2-line ellipsized; RichText plain-preview + "rich" badge; Image thumbnail + dimensions; Uris count + first. Keyboard: ↑↓ Enter(paste) Esc(close) Ctrl+F(search) Del(delete) Ctrl+P(pin) Tab(cycle). Mouse: click pastes; hover actions pin/delete/copy; context menu Paste / Paste as plain text / Copy / Pin / Delete. Delete → 3s undo-toast. Empty states defined. 150ms fade+scale open/close. On pick: close immediately, paste async. Focus lands on the list.

**Settings window (v1 scope)**: shortcut (re-registers per DE), history size, image size cap, autostart toggle, extension status + install button, "Clear all data". Advanced knobs stay in config file only.

## 7. First-run, shortcuts, doctor

`ua-clipboard setup` (idempotent): detect DE → register Super+V (gsettings / portal / emit snippet) → write XDG autostart `.desktop` → Hyprland data-control check → offer extension install (GNOME) → start daemon. `ua-clipboard doctor`: checks shortcut registered, autostart present, daemon running, DBus reachable, watcher active per environment, extension present (GNOME), secrets-filter stats. Uninstall reverses registrations.

## 8. Reliability engineering

- All policy logic in `ua-core`, 100% unit-tested (no OS deps).
- Paste pipeline tested against a fake clock + fake focus/selection emitter.
- CI (GitHub Actions, ubuntu): `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, release build. X11 integration test under Xvfb (copy via xclip, assert captured).
- Extension: version-pin `metadata.json` per GNOME minor; CI greps new GNOME releases.
- Daemon: panic-hook logs to `~/.local/state/ua-clipboard/daemon.log`; systemd user unit optional for auto-restart.

## 9. i18n, theming, a11y

gettext from day one (`t()` module; en shipped, ar-ready); GTK4 mirrors RTL automatically. Theming follows system via libadwaita `color-scheme`. v1 a11y: full keyboard operation + accessible labels; screen-reader polish is v2.

## 10. Packaging

crates.io (`cargo install ua-clipboard` / binstall) → `install.sh` → AUR (`ua-clipboard`, `-git`) → GitHub Releases (.deb via cargo-deb, .rpm via cargo-generate-rpm) → COPR. **No Flatpak** (sandbox blocks watching/shortcuts/autostart).

## 11. v1 acceptance checklist

- [ ] Copy text/image/HTML/URIs on GNOME(Wayland+X11), KDE, sway → appears in panel ≤ 300ms
- [ ] Super+V opens panel on all five environments after `setup`, zero manual steps
- [ ] Pick with Enter or click → pastes into previously focused app (all envs; GNOME requires extension)
- [ ] Pin survives Clear-all, reboot; 100-entry cap evicts oldest unpinned
- [ ] KeePassXC-marked secret copy never stored (verifiable via `doctor` counters)
- [ ] Search filters instantly, case-insensitive, highlighted
- [ ] Uninstall removes shortcut, autostart, data (with confirm)
- [ ] `cargo test` green; clippy clean; Xvfb integration green

## 12. v2 candidates

Sync across devices (E2E), primary-selection capture, paste-as-plain-text transforms, screen-reader polish, per-app capture rules.
