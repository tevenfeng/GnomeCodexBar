import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';
import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';
import {ConfigManager} from './configManager.js';

export default class CodexBarPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const cfg = new ConfigManager();

        // ─── General page ───
        const genPage = new Adw.PreferencesPage({
            title: _('General'),
            icon_name: 'dialog-information-symbolic',
        });
        window.add(genPage);

        const refreshGrp = new Adw.PreferencesGroup({ title: _('Refresh Settings') });
        genPage.add(refreshGrp);

        const makeErrorLabel = () => new Gtk.Label({
            visible: false,
            wrap: true,
            xalign: 0,
            css_classes: ['error'],
        });
        const refreshErrorLabel = makeErrorLabel();
        const providerErrorLabel = makeErrorLabel();
        const showError = message => {
            refreshErrorLabel.label = message;
            refreshErrorLabel.visible = true;
            providerErrorLabel.label = message;
            providerErrorLabel.visible = true;
        };
        const clearError = () => {
            refreshErrorLabel.label = '';
            refreshErrorLabel.visible = false;
            providerErrorLabel.label = '';
            providerErrorLabel.visible = false;
        };

        // Refresh interval: read from config.toml, fallback to GSettings
        const settings = this.getSettings();
        let interval = cfg.getRefreshInterval();
        if (interval === null) interval = settings.get_int('refresh-interval');

        const refreshRow = new Adw.SpinRow({
            title: _('Refresh Interval (seconds)'),
            subtitle: _('How often the CLI daemon fetches usage data'),
            adjustment: new Gtk.Adjustment({
                lower: 30, upper: 3600, step_increment: 30, value: interval,
            }),
        });
        refreshRow.connect('changed', () => {
            if (!cfg.setRefreshInterval(refreshRow.value)) {
                log('[codex-bar] Failed to save refresh interval');
                showError(_('Failed to save refresh interval.'));
            } else {
                clearError();
            }
        });
        refreshGrp.add(refreshRow);

        const errorRow = new Adw.ActionRow({ visible: false });
        errorRow.add_suffix(refreshErrorLabel);
        refreshGrp.add(errorRow);
        refreshErrorLabel.bind_property('visible', errorRow, 'visible', Gio.BindingFlags.SYNC_CREATE);

        const cliPathRow = new Adw.EntryRow({ title: _('CLI Path') });
        settings.bind('cli-path', cliPathRow, 'text', Gio.SettingsBindFlags.DEFAULT);
        refreshGrp.add(cliPathRow);

        // ─── Providers page ───
        const provPage = new Adw.PreferencesPage({
            title: _('Providers'),
            icon_name: 'network-server-symbolic',
        });
        window.add(provPage);

        const provGrp = new Adw.PreferencesGroup({ title: _('Enabled Providers') });
        provPage.add(provGrp);

        const providerErrorRow = new Adw.ActionRow({ visible: false });
        providerErrorRow.add_suffix(providerErrorLabel);
        provGrp.add(providerErrorRow);
        providerErrorLabel.bind_property('visible', providerErrorRow, 'visible', Gio.BindingFlags.SYNC_CREATE);

        const dsRow = new Adw.SwitchRow({
            title: _('DeepSeek'),
            subtitle: _('Balance and usage monitoring'),
        });
        dsRow.set_active(cfg.isProviderEnabled('deepseek'));
        dsRow.connect('notify::active', () => {
            if (!cfg.setProviderEnabled('deepseek', dsRow.active)) {
                log('[codex-bar] Failed to save DeepSeek enabled state');
                showError(_('Failed to save DeepSeek enabled state.'));
            } else {
                clearError();
            }
        });
        provGrp.add(dsRow);

        const sfRow = new Adw.SwitchRow({
            title: _('StepFun'),
            subtitle: _('Step Plan rate limits and reset times'),
        });
        sfRow.set_active(cfg.isProviderEnabled('stepfun'));
        sfRow.connect('notify::active', () => {
            if (!cfg.setProviderEnabled('stepfun', sfRow.active)) {
                log('[codex-bar] Failed to save StepFun enabled state');
                showError(_('Failed to save StepFun enabled state.'));
            } else {
                clearError();
            }
        });
        provGrp.add(sfRow);

        const ocgRow = new Adw.SwitchRow({
            title: _('OpenCode Go'),
            subtitle: _('5h, weekly, and monthly coding plan limits'),
        });
        ocgRow.set_active(cfg.isProviderEnabled('opencodego'));
        ocgRow.connect('notify::active', () => {
            if (!cfg.setProviderEnabled('opencodego', ocgRow.active)) {
                log('[codex-bar] Failed to save OpenCode Go enabled state');
                showError(_('Failed to save OpenCode Go enabled state.'));
            } else {
                clearError();
            }
        });
        provGrp.add(ocgRow);
    }
}
