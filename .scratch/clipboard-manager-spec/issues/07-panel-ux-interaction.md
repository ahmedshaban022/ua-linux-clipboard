# 07 — Panel UX and interaction model

Label: wayfinder:grilling
Type: grilling
Status: resolved

## Question

Specify the panel as the user experiences it: layout, search, pin/delete affordances, keyboard model, empty states, animations.

## Answer

**Panel spec (Win+V feel, fixes its gaps):**

- Size ~420×560dp, rounded (16px), header with search field (instant case-insensitive substring, matches anywhere; highlighted matches), list below.
- Two sections: **Pinned** (top, pin icon), **History** (newest first, relative timestamps, source-app name). Pins survive Clear-all and never age out.
- Cards: text → 2-line ellipsized preview; HTML → plain-text preview with a small "rich" badge; images → thumbnail (≤96dp, contain) + dimensions; URIs → count + first item.
- Keyboard: ↑/↓ move, Enter paste, Esc close (focus returns), Ctrl+F focus search, Del delete, Ctrl+P pin/unpin, Tab cycles search/list/sections. Mouse: click pastes; hover shows pin/delete/copy actions; right-click menu (Paste / Paste as plain text / Copy / Pin / Delete).
- Delete = per-item X with undo-toast (3s); "Clear all" in the header menu (keeps pins, asks confirm).
- Empty state: friendly illustration + "Copy something to get started"; search-empty: "No matches for 'x'".
- Opens with keyboard focus on the list (search one Ctrl+F away — instant-type like Win+V). Animations: 150ms fade+scale on open/close only.
- On pick: the panel closes immediately (perceived speed); the paste pipeline runs async with a fallback toast on failure.

Resolved by user delegation ("implement all tickets") 2026-10-08, consistent with the charting decisions (substring search, pin behavior).
