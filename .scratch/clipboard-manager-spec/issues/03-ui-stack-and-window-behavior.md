# 03 — Which UI stack, and what window behavior is possible per compositor?

Label: wayfinder:research
Type: research
Status: resolved

## Question

For a Win11-grade flyout panel in Rust with low idle RAM: which UI stack, and what window behavior is possible per compositor?

## Answer

**Stack: Rust + gtk4-rs + libadwaita, vanilla (no relm4).** Rationale: GNOME-first native look (Adwaita is the Fluent-equivalent design language on GNOME: rounded, dark/light, accents); runs fine on KDE/wlroots/X11; best-documented Rust GUI path; startup and idle RAM well within target. Qt6/QML via cxx-qt rejected: heavier FFI seam, GNOME look suffers (user priority is GNOME). Slint/Iced rejected: weaker desktop integration (portals, themes, IME) for a system utility. (Comparative benchmarks UNVERIFIED — search rate-limited; decision robust regardless.)

**Window behavior matrix:**

| Compositor | Strategy |
|---|---|
| GNOME (extension present) | Extension shows/toggles the flyout anchored near focus — true Win11 feel |
| GNOME (no extension) | Centered/last-position libadwaita window (mutter forbids self-positioning and layer-shell) — graceful degrade |
| KDE / wlroots | `wlr-layer-shell` overlay positioned near pointer (KWin and wlroots implement it; mutter does not) |

xdg-activation-v1 used to request focus properly when showing the window.

**i18n/RTL:** gettext for strings; GTK4 mirrors layout automatically under RTL locales — the panel is list-based, no custom mirroring needed. All strings live behind a translation module from day one.

**GNOME extension maintenance:** GJS, standard extension API (St/Clutter/Shell), version-pinned `metadata.json` per GNOME release — the GPaste-proven model. Rust-in-shell not mature enough (UNVERIFIED beyond 2024 knowledge).
