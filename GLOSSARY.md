# UA Clipboard

A Windows 11-style clipboard history manager for Linux (GNOME first, KDE and wlroots too; Wayland and X11).

## Language

**Entry**:
One captured clipboard item, of kind Text, RichText (HTML), Image, or Uris.
_Avoid_: clip, record, item

**History**:
The time-ordered collection of unpinned Entries, newest first.
_Avoid_: log, backlog

**Pin**:
A marker that exempts an Entry from eviction and from Clear-all.
_Avoid_: favorite, star

**Panel**:
The flyout window that lists Pins and History and triggers a paste on pick.
_Avoid_: popup, dialog

**Daemon**:
The resident process that watches the clipboard, owns storage, and serves clients.
_Avoid_: service, background app

**Watcher**:
The environment-specific monitor that feeds clipboard content to the Daemon.
_Avoid_: listener

**Companion Extension**:
The GNOME Shell extension that watches, anchors the Panel near focus, and injects pastes on GNOME.
_Avoid_: plugin, add-on

**Paste Pipeline**:
The ordered steps from pick to injected Ctrl+V: focus-return → selection set → ownership confirm → injection.
_Avoid_: paste flow

**Secret**:
Clipboard content marked sensitive by its source app; never stored.
_Avoid_: password (Secrets are broader than passwords)

**Doctor**:
The diagnostic command that checks setup health per environment.
_Avoid_: troubleshooter

**Plain-text Paste**:
A paste variant that drops HTML and pastes only the text representation.
_Avoid_: strip paste
