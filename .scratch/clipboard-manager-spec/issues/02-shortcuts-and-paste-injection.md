# 02 — How do Super+V registration and auto-paste injection work per environment?

Label: wayfinder:research
Type: research
Status: resolved

## Question

Per environment (GNOME Wayland, KDE Plasma 6, wlroots, X11), as of October 2026: how to register a remappable global shortcut defaulting to Super+V, and how to inject Ctrl+V after the panel closes?

## Answer

**Capability matrix (verified Oct 2026):**

| Environment | Shortcut registration | Ctrl+V injection |
|---|---|---|
| GNOME Wayland | **gsettings custom-keybindings** (programmatic: `org.gnome.settings-daemon.plugins.media-keys custom-keybindings/customN` = name/command/binding `<Super>v`; verified working recipe). Alternative: GlobalShortcuts portal (GNOME 46+). Super alone unbindable; Super+V fine | **Companion extension**: set selection via StClipboard, then synthesize Ctrl+V via Clutter `VirtualInputDevice` (the proven Clipboard-Indicator mechanism) |
| KDE Plasma 6 | **GlobalShortcuts portal** (supported; the EasyEffects route); kglobalaccel DBus is internal/unstable for external apps | Portal-assisted or KWin virtual-keyboard; Xwayland fallback |
| wlroots | compositor config binds (`bindsym`/`bind`), first-run emits snippet; portal where implemented | `zwp_virtual_keyboard_manager_v1` (wtype mechanism) |
| X11 | **XGrabKey** via x11rb (mask out NumLock/ScrollLock modifiers) | **XTest fake input** via x11rb |

**Reliability (the "sometimes works, sometimes not" cure):** Clipboard-Indicator [issue #433](https://github.com/Tudmotu/gnome-shell-extension-clipboard-indicator) (older items fail to paste) is the known fragility: the synthetic Ctrl+V races the selection handover and focus return. Our design: state machine — (1) panel closes, (2) wait for focus-return signal, (3) set selection, (4) wait for selection-ownership confirmation, (5) inject Ctrl+V, with bounded retries and a user-visible fallback ("clipboard set — press Ctrl+V") if any step times out. Fallback tool for locked-down setups: `ydotool` (kernel uinput; one-time udev rule) — documented, not required.

**Recommendation:** default Super+V everywhere via per-DE registration at first run; remappable in settings (re-registers).
