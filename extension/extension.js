// UA Clipboard — GNOME Shell companion extension (experimental skeleton).
//
// Why this exists (ADR-0001): Mutter implements neither wlr-data-control
// nor ext-data-control, so a daemon cannot watch the clipboard on GNOME
// Wayland. This extension is the watcher: it polls St.Clipboard, and pushes
// every new text to the daemon over its Unix socket (JSON-line protocol).
//
// Next milestones (tracked in the README status table):
//   1. Toggle the panel anchored near the focus on Super+V.
//   2. Paste injection: set the selection via St.Clipboard, then synthesize
//      Ctrl+V through a Clutter VirtualInputDevice (Clipboard-Indicator
//      mechanism), driven by the daemon's paste state machine.
//
// Install: cp -r extension/ ~/.local/share/gnome-shell/extensions/ua-clipboard@ua/
// then restart GNOME Shell and enable via Extension Manager.

import St from 'gi://St';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const POLL_MS = 700;

export default class UAClipboardExtension extends Extension {
    enable() {
        this._last = '';
        this._clipboard = St.Clipboard.get_default();
        this._timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, POLL_MS, () => {
            this._poll();
            return GLib.SOURCE_CONTINUE;
        });
        console.log('[ua-clipboard] extension enabled (watching → daemon socket)');
    }

    disable() {
        if (this._timeout) {
            GLib.source_remove(this._timeout);
            this._timeout = null;
        }
        this._clipboard = null;
        this._last = '';
    }

    _poll() {
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (cb, text) => {
            if (text && text !== this._last) {
                this._last = text;
                this._push({ type: 'capture', offers: [], text: text, html: null, image_b64: null, uris: null, source_app: null });
            }
        });
    }

    _push(request) {
        try {
            const runtime = GLib.getenv('XDG_RUNTIME_DIR') || '/tmp';
            const client = new Gio.SocketClient();
            const conn = client.connect(
                Gio.UnixSocketAddress.new(`${runtime}/ua-clipboard.sock`), null);
            const out = conn.get_output_stream();
            out.write_all(`${JSON.stringify(request)}\n`, null);
            out.flush(null);
            conn.close(null);
        } catch (e) {
            // Daemon not running: expected during development; stay quiet
            // unless debugging.
            console.debug(`[ua-clipboard] push failed: ${e.message}`);
        }
    }
}
