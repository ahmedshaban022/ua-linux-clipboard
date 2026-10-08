# 06 — GNOME Wayland companion strategy for auto-paste and flyout positioning

Label: wayfinder:grilling
Type: grilling
Status: resolved

## Question

Given the research on input injection and window positioning, choose the GNOME Wayland story.

## Answer

**The companion GJS extension is load-bearing, not optional.** Mutter blocks clipboard-watching protocols entirely, so on GNOME the extension is also the *watcher* (not just paste/positioning) — this is why GPaste and Clipboard Indicator ship extensions, and why pure-daemon tools are flaky on GNOME. (User had already chosen "Extension in v1" in charting round 1.)

v1 scope of the extension: (1) watch clipboard via StClipboard, forward entries to the daemon over DBus; (2) toggle the panel anchored near focus on Super+V (registered via gsettings running `ua-clipboard toggle`, or the extension's own shortcut via its prefs); (3) on pick: set the selection + Clutter VirtualInputDevice Ctrl+V through the focus-return/selection-confirmation state machine (the anti-#433 design from ticket 02). Without the extension the app still works (centered window, manual Ctrl+V) — degrade, never break.

ydotool documented as the no-extension fallback; RemoteDesktop DBus rejected (fragile permission story). Extension maintenance: version-pin `metadata.json` per GNOME minor, CI check on new GNOME releases.
