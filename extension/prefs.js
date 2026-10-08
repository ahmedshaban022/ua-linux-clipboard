// UA Clipboard — minimal prefs (placeholder until the settings milestone
// wires shortcut + behavior options into the extension).

import Gtk from 'gi://Gtk';
import { ExtensionPreferences } from 'resource:///org/gnome/shell/extensions/extension.js';

export default class UAClipboardPrefs extends ExtensionPreferences {
    getPreferencesWidget() {
        const box = new Gtk.Box({ orientation: Gtk.Orientation.VERTICAL, spacing: 12, margin_top: 12, margin_bottom: 12, margin_start: 12, margin_end: 12 });
        box.append(new Gtk.Label({ label: 'UA Clipboard\n\nThe companion extension is configured via the main app:\n\n  ua-clipboard settings\n  ua-clipboard doctor\n', halign: Gtk.Align.START, wrap: true }));
        return box;
    }
}
