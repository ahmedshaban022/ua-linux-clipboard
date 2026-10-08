# 05 — Lock the language, toolkit, and process architecture

Label: wayfinder:grilling
Type: grilling
Status: resolved

## Question

With the research in hand, decide what the spec commits to: language, UI toolkit, process model, IPC, module layout.

## Answer

**Locked:** Rust workspace, five crates —

- `ua-core`: pure domain logic — entry model, history policy (dedup consecutive, caps, eviction), pin logic, substring search, password-hint filtering, settings. No UI, no OS calls. Unit-testable anywhere.
- `ua-daemon`: the always-running process. `ClipboardWatcher` trait with three impls (GNOME-extension-DBus, wl-clipboard-rs, x11rb-XFixes); owns SQLite; exposes control via DBus (`org.ua.Clipboard`) + Unix socket fallback; emits signals on new entries.
- `ua-gtk`: the panel — gtk4-rs + libadwaita window (search bar, card list, pin/delete, keyboard model); talks to the daemon; on pick → asks the daemon to set the selection, then runs the paste pipeline for the environment.
- `ua-cli`: `ua-clipboard toggle|list|pin|delete|setup|doctor` — first-run setup and diagnostics.
- `extension/`: GJS GNOME Shell companion (watch, flyout positioning, paste injection) talking DBus to the daemon.

Process model: the daemon is the single source of truth; the panel and CLI are thin clients. Rationale: watching must never die with the panel window; matches clipcat's proven shape with our extension twist for GNOME. Decided by user delegation ("implement all tickets") on 2026-10-08, consistent with all prior grilling answers.
