# Wayfinder map: Windows 11-style clipboard manager for Linux

Label: wayfinder:map

## Destination

A single, complete, implementation-ready spec at `.scratch/clipboard-manager-spec/spec.md` for a Windows 11 Win+V-style clipboard manager on Linux (GNOME first, KDE Plasma and other common desktops; both Wayland and X11). Every v1 decision settled — stack, architecture, panel UX, shortcut registration, auto-paste strategy, storage, packaging — so the spec can be handed to `/to-tickets` and built without another design conversation.

## Notes

- Aligned with user (2026-10-08): top priorities are **reliability** ("never misses a copy, sometimes works sometimes not" is why GPaste/CopyQ were rejected) and **Win11-grade UI polish**.
- Core behavior: history of text + images as preview cards, pin (survives clear-all and reboots), `Super+V` default shortcut (remappable), **auto-paste on pick** (user requires it; accepts a companion helper on GNOME Wayland), autostart with session, follows system light/dark, keyboard + mouse navigation, lightweight (low idle RAM, instant panel).
- v1 extras: search/filter bar; password-manager awareness (content marked by password managers is never stored).
- Out of v1: sync across devices. UI language: English first, architecture i18n/RTL-ready.
- Quality bar: best practices, high performance. Leaning Rust for the core — to be locked by ticket 05.
- Settled in charting (2026-10-08): product name **UA Clipboard**; history defaults 100 entries with images up to 16MB each (all configurable; pins never age out); search is an instant case-insensitive substring filter; GNOME Wayland v1 ships with the companion GNOME Shell extension for auto-paste + flyout positioning (exact mechanism confirmed by ticket 06 from research).
- Skills: HITL tickets use `grilling` + `domain-modeling`; research tickets are resolved by subagents calling the `research` skill.
- Tracker: local markdown (see `docs/agents/issue-tracker.md`). Repo has no git yet, so research findings land directly in each ticket's `## Answer` section.

## Decisions so far

- [How do we watch the clipboard on every target?](issues/01-watch-clipboard-sources.md): Mutter blocks the clipboard protocols, so GNOME watches via the companion extension over DBus; KDE/wlroots via wl-clipboard-rs (ext/wlr-data-control); X11 via x11rb XFixes — one `ClipboardWatcher` trait, runtime selection.
- [How do Super+V registration and auto-paste injection work per environment?](issues/02-shortcuts-and-paste-injection.md): gsettings custom-keybindings (GNOME), GlobalShortcuts portal (KDE), XGrabKey (X11); paste via Clutter injection / XTest / virtual-keyboard behind a focus-return + selection-confirm state machine with fallback toast.
- [Which UI stack, and what window behavior is possible per compositor?](issues/03-ui-stack-and-window-behavior.md): Rust + gtk4-rs + libadwaita; extension-anchored flyout on GNOME, layer-shell on KDE/wlroots, centered degrade without extension; gettext i18n, RTL automatic.
- [How do we store history and ship the app?](issues/04-storage-and-packaging.md): rusqlite bundled, image blobs + thumbnails, WAL; Flatpak ruled out — native packages only; XDG autostart.
- [Lock the language, toolkit, and process architecture](issues/05-stack-architecture-lock.md): five-crate Rust workspace (`ua-core`, `ua-daemon`, `ua-gtk`, `ua-cli`, `extension/`); daemon is the single source of truth, DBus `org.ua.Clipboard`.
- [GNOME Wayland companion strategy](issues/06-gnome-companion-strategy.md): the extension is load-bearing (watch + position + inject); without it the app degrades to centered window + manual Ctrl+V, never breaks.
- [Panel UX and interaction model](issues/07-panel-ux-interaction.md): Pinned/History sections, card previews per kind, full keyboard model, undo-toast deletes, 150ms open/close animation, close-then-paste async pipeline.
- [Packaging and distribution plan](issues/08-packaging-distribution.md): crates.io → install.sh → AUR → Releases (.deb/.rpm) → COPR; `doctor` command diagnoses setup per DE.
- [Panel UI prototype](issues/09-panel-ui-prototype.md): superseded — the first `ua-gtk` build is the prototype the user reacts to.

## Not yet specified

— none: destination reached. The spec is written at [spec.md](spec.md); residual detail (accessibility polish, automated cross-compositor harness) is scoped inside the spec's v1/v2 boundaries.

## Out of scope

- Cloud sync across devices (v2 candidate)
- Windows / macOS / mobile clients
- AI-powered actions on clipboard content
