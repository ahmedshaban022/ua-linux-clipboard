# The GNOME companion extension is load-bearing, not optional

On GNOME Wayland, Mutter deliberately implements neither `wlr-data-control-unstable-v1` nor its standardized successor `ext-data-control-v1` (privacy stance; still true through GNOME 49/50 as of Oct 2026 — mutter#524). A clipboard daemon therefore **cannot watch the clipboard there via Wayland protocols**, cannot position a flyout near the user's focus, and cannot inject Ctrl+V. Our GJS GNOME Shell extension does all three (StClipboard watching, Clutter-anchored panel, VirtualInputDevice paste) and talks to the daemon over DBus.

**Considered options**: ydotool-only (rejected: needs uinput permissions and still can't watch or position), org.gnome.Mutter.RemoteDesktop DBus (rejected: fragile permission story). Without the extension the app degrades to a centered window + manual Ctrl+V — degrade, never break.

**Consequences**: the extension must be version-pinned per GNOME minor release (`metadata.json`), with a CI check on new GNOME releases. This is also why pure-daemon clipboard managers are flaky on GNOME — the "sometimes works, sometimes not" this project exists to fix.
