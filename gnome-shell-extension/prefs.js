import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';
import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

export default class CodexBarPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const page = new Adw.PreferencesPage({
            title: _('General'),
            icon_name: 'dialog-information-symbolic',
        });
        window.add(page);

        const group = new Adw.PreferencesGroup({
            title: _('Refresh Settings'),
        });
        page.add(group);

        const settings = this.getSettings();

        const refreshRow = new Adw.SpinRow({
            title: _('Refresh Interval'),
            adjustment: new Gtk.Adjustment({
                lower: 30,
                upper: 3600,
                step_increment: 30,
            }),
        });
        settings.bind('refresh-interval', refreshRow, 'value', Gio.SettingsBindFlags.DEFAULT);
        group.add(refreshRow);

        const cliPathRow = new Adw.EntryRow({
            title: _('CLI Path'),
        });
        settings.bind('cli-path', cliPathRow, 'text', Gio.SettingsBindFlags.DEFAULT);
        group.add(cliPathRow);
    }
}
