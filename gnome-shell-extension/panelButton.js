import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

let _button = null;

export function createPanelButton() {
    if (_button) return _button;

    _button = new PanelMenu.Button(0.5, 'Codex Bar', false);

    const box = new St.BoxLayout({});
    _button.add_child(box);

    _button._shortName = new St.Label({
        text: '',
        y_align: Clutter.ActorAlign.CENTER,
        style: 'font-size: 10px; margin-right: 4px; color: #888;',
    });
    box.add_child(_button._shortName);

    _button._icon = new St.Label({
        text: '\u25CF',
        y_align: Clutter.ActorAlign.CENTER,
        style: 'font-size: 10px; margin-right: 4px;',
    });
    box.add_child(_button._icon);

    _button._label = new St.Label({
        text: '--',
        y_align: Clutter.ActorAlign.CENTER,
        style_class: 'codex-bar-panel-label',
    });
    box.add_child(_button._label);

    Main.panel.addToStatusArea('codex-bar', _button);
    return _button;
}

export function updatePanelButton(providerData, providerId) {
    if (!_button) return;

    const shortName = providerId === 'deepseek' ? 'DS' : 'SF';
    _button._shortName.text = shortName;

    if (!providerData || providerData.error) {
        _button._icon.style = 'font-size: 10px; margin-right: 4px; color: #cc3333;';
        _button._label.text = 'ERR';
        _button._label.style_class = 'codex-bar-panel-label low';
        return;
    }

    if (providerId === 'deepseek') {
        const d = providerData.details || {};
        const total = d.total_balance != null ? Number(d.total_balance) : 0;
        const currency = d.currency || 'CNY';
        const symbol = currency === 'USD' ? '$' : '\u00A5';
        let display = `${symbol}${total.toFixed(2)}`;
        if (display.length > 10) display = `${symbol}${Math.round(total)}`;
        _button._icon.style = 'font-size: 10px; margin-right: 4px; color: #3584e4;';
        _button._label.text = display;
        _button._label.style_class = 'codex-bar-panel-label high';
        return;
    }

    const pct = Math.round(providerData.remaining_percent || 0);
    let cls = 'high', dot = '#33cc66';
    if (pct < 20) { cls = 'low'; dot = '#cc3333'; }
    else if (pct < 50) { cls = 'medium'; dot = '#e6a817'; }
    _button._icon.style = `font-size: 10px; margin-right: 4px; color: ${dot};`;
    _button._label.text = `${pct}%`;
    _button._label.style_class = `codex-bar-panel-label ${cls}`;
}

export function getPanelButton() {
    return _button;
}

export function destroyPanelButton() {
    if (_button) {
        _button.destroy();
        _button = null;
    }
}
