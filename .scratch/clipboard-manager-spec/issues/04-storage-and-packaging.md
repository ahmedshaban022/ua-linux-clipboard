# 04 — How do we store history and ship the app?

Label: wayfinder:research
Type: research
Status: resolved

## Question

How do we store history (text + images, pins surviving reboots) and distribute the app?

## Answer

**Storage:** SQLite via `rusqlite` (bundled feature → no system dep). Schema: single `entries` table (id, kind, text, html, image_blob, thumb_blob, source_app, copied_at, pinned, pin_order, preview) + `meta`. Images as **blobs** (simpler backup story; DB stays small due to caps: 100 entries default, ≤16MB/image, thumbnails generated on insert). Pins never evicted. WAL mode for concurrent reads while the daemon writes. ([clipcat](https://github.com/xrelkd/clipcat) validates SQLite-with-images at scale.)

**Flatpak: confirmed dead end** for this app class — the sandbox blocks clipboard monitoring, global shortcut registration, and clean autostart (CopyQ's Flatpak ships documented limitations). Native packaging only. (Rate-limited search; consistent with long-standing constraints — re-verify opportunistically.)

**Channels (order):** (1) crates.io via `cargo install` / `cargo-binstall`, (2) `install.sh` source-build script, (3) AUR, (4) COPR + .deb (cargo-deb) + .rpm (cargo-generate-rpm). GitHub Releases carry prebuilt binaries.

**Autostart:** XDG autostart `.desktop` in `~/.config/autostart` (universal, user-scope, no root). Optional systemd user unit later.

**First-run:** detects DE, registers Super+V (gsettings on GNOME; portal on KDE; emits config snippet on wlroots), installs the autostart entry, checks Hyprland data-control enablement, offers the GNOME extension install. All steps idempotent; uninstall reverses them.
