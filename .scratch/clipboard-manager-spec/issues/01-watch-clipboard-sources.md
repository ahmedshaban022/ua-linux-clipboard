# 01 — How do we watch the clipboard on every target?

Label: wayfinder:research
Type: research
Status: resolved

## Question

How should the daemon capture clipboard changes on GNOME Wayland, KDE Plasma 6 Wayland, wlroots compositors (sway/Hyprland), and X11, as of October 2026?

## Answer

**Watching matrix (verified Oct 2026):**

| Environment | Method | Status |
|---|---|---|
| GNOME Wayland | Mutter supports **neither** `wlr-data-control` nor `ext-data-control` (deliberate privacy stance, still true through GNOME 49/50 per reports) → the **GNOME Shell companion extension watches the clipboard** (StClipboard/JS) and forwards entries to the daemon over DBus | VERIFIED ([mutter#524](https://gitlab.gnome.org/GNOME/mutter/-/issues/524), GNOME Discourse, Strata write-up) |
| KDE Plasma 6 Wayland | KWin implements **`ext-data-control-v1`**; `wl-clipboard-rs` ≥0.9.2 speaks it, watch/multiplex API available | VERIFIED ([protocol table](https://absurdlysuspicious.github.io/wayland-protocols-table)) |
| wlroots (sway/Hyprland) | `wlr-data-control-unstable-v1`. **Gotcha:** Hyprland ships data-control disabled by default (security) — must be enabled in compositor config; first-run must detect and instruct | VERIFIED (wl-clipboard-rs GitHub issue, Jul 2025) |
| X11 | **XFixes selection-notify events** via `x11rb` (event-driven, no polling) | Standard, stable |

**Crates:** `wl-clipboard-rs` 0.9.x actively maintained, ext-data-control since 0.9.2; `x11rb` for XFixes+XTest; `arboard` not suitable for monitoring (pull-only).

**MIME formats to capture:** `text/plain;charset=utf-8`, `text/html`, `image/png`, `text/uri-list`; honor `x-kde-passwordManagerHint` (Klipper/KeePassXC convention) to never store secrets (partially verified — confirm exact MIME string during implementation).

**clipcat lessons** ([xrelkd/clipcat](https://github.com/xrelkd/clipcat): Rust, client-server, SQLite with images): validates daemon+client+SQLite; its Wayland support is "experimental" — the gap we close is GNOME via extension + first-class UX.

**Recommendation:** trait `ClipboardWatcher` with three impls (extension-DBus on GNOME, wl-clipboard-rs on KDE/wlroots, x11rb on X11); daemon picks at runtime by `XDG_SESSION_TYPE` + compositor detection.
