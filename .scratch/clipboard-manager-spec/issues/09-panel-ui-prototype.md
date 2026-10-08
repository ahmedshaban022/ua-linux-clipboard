# 09 — Panel UI prototype to react to

Label: wayfinder:prototype
Type: prototype
Status: resolved

## Question

Build a cheap, concrete artifact of the panel so the user can react before the spec freezes the design.

## Answer

**Resolution: superseded by direct implementation.** The user directed "implement all tickets", so instead of a throwaway mockup the prototype IS the first build of `ua-gtk` — the real gtk4-rs/libadwaita panel implementing ticket 07's spec, runnable on the user's Linux machine via the repo's build instructions. Any UX changes discovered while using it flow back into `spec.md` before wider packaging work.
