# 08 — Packaging and distribution plan

Label: wayfinder:grilling
Type: grilling
Status: resolved

## Question

Which install channels first, and how does first-run set up autostart + Super+V per DE?

## Answer

**v1 channels (order of work):** crates.io (`cargo install ua-clipboard` / cargo-binstall prebuilt) → `install.sh` (builds from source, runs `ua-clipboard setup`) → AUR (`ua-clipboard`, `-git`) → GitHub Releases with prebuilt .deb/.rpm (cargo-deb / cargo-generate-rpm) → COPR. Flatpak: no (ticket 04).

Autostart via XDG autostart `.desktop` written by `setup`. `ua-clipboard doctor` diagnoses per-DE: shortcut registered? extension installed? data-control enabled (Hyprland)? daemon running? DBus reachable?

Own-machine story = install.sh; public story = crates.io + AUR + Releases. Uninstall reverses all registrations. Resolved by user delegation ("implement all tickets") 2026-10-08.
