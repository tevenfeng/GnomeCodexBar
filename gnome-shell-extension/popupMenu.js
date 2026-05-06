import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import { readStatus, readSelectedProvider, writeSelectedProvider } from './statusReader.js';
import { updatePanelButton } from './panelButton.js';

const CFG = {
    deepseek: { name: 'DeepSeek', color: '#3584e4' },
    stepfun: { name: 'StepFun', color: '#ff8c00' },
};

let _currentSelection = 'deepseek';

export function initMenu(menu) {
    _currentSelection = readSelectedProvider();
    menu.connect('open-state-changed', (m, open) => {
        if (open) _rebuild(menu);
    });
}

export function rebuildMenu(menu) {
    _currentSelection = readSelectedProvider();
    _rebuild(menu);
}

function _rebuild(menu) {
    menu.removeAll();

    // Add our class to the existing popup styling (don't replace!)
    menu.actor.add_style_class_name('codex-bar-popup');

    const status = readStatus();

    if (!status || !status.providers) {
        const item = new PopupMenu.PopupBaseMenuItem({ reactive: false });
        item.add_child(new St.Label({
            text: 'No data.\nRun "codex-bar-cli daemon" first.',
            style: 'font-size: 11px; color: #999; padding: 12px 16px;',
        }));
        menu.addMenuItem(item);
        _addRefreshBtn(menu);
        return;
    }

    for (const provider of status.providers) {
        _addProviderItem(menu, provider, status);
    }

    menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

    _addRefreshBtn(menu);
}

function _addProviderItem(menu, provider, status) {
    const pid = provider.provider_id;
    const cfg = CFG[pid] || { name: pid, color: '#888' };
    const isSelected = pid === _currentSelection;

    const item = new PopupMenu.PopupBaseMenuItem({
        reactive: true,
        can_focus: true,
    });

    // Remove default styling and replace with ours
    item.actor.style = isSelected
        ? 'padding: 0; margin: 4px 12px; border: 1px solid rgba(53,132,228,0.25); border-radius: 8px; background-color: rgba(53,132,228,0.12);'
        : 'padding: 0; margin: 4px 12px; border: 1px solid rgba(255,255,255,0.06); border-radius: 8px; background-color: rgba(255,255,255,0.05);';

    const box = new St.BoxLayout({ vertical: true });

    // Header row
    const header = new St.BoxLayout({ style: 'padding: 10px 12px 6px 12px;' });
    header.add_child(new St.Label({
        text: isSelected ? '\u25C9' : '\u25CB',
        style: isSelected ? 'color: #3584e4; width: 20px; font-size: 15px;' : 'color: #666; width: 20px; font-size: 15px;',
    }));
    header.add_child(new St.Label({
        text: cfg.name,
        style: isSelected ? 'color: #3584e4; font-size: 13px; font-weight: 600;' : 'color: #ccc; font-size: 13px; font-weight: 600;',
    }));
    box.add_child(header);

    if (provider.error) {
        box.add_child(new St.Label({
            text: `Error: ${provider.error}`,
            style: 'font-size: 11px; color: #cc3333; padding: 0 12px 8px 32px;',
        }));
    } else {
        _addDetails(box, provider);
    }

    item.add_child(box);

    // Click to select provider
    item.connect('button-press-event', () => {
        _currentSelection = pid;
        writeSelectedProvider(pid);

        if (status && status.providers) {
            const p = status.providers.find(pr => pr.provider_id === pid);
            updatePanelButton(p || null, pid);
        }

        // Rebuild menu to reflect selection
        _rebuild(menu);
        return Clutter.EVENT_STOP;
    });

    menu.addMenuItem(item);
}

function _addDetails(box, provider) {
    const pid = provider.provider_id;
    const d = provider.details || {};

    if (pid === 'deepseek') {
        const cur = d.currency || 'CNY';
        const sym = cur === 'USD' ? '$' : '\u00A5';
        for (const [label, key] of [
            ['Balance', 'total_balance'],
            ['Granted', 'granted_balance'],
            ['Topped Up', 'topped_up_balance'],
        ]) {
            const val = d[key] != null ? Number(d[key]) : 0;
            const row = new St.BoxLayout({ style: 'padding: 2px 12px 2px 32px;' });
            row.add_child(new St.Label({ text: label + ': ', style: 'font-size: 11px; color: #888;' }));
            row.add_child(new St.Label({ text: `${sym}${val.toFixed(2)}`, style: 'font-size: 11px; color: #bbb; font-family: monospace;' }));
            box.add_child(row);
        }
        box.add_child(new St.Widget({ style: 'height: 6px;' }));
        return;
    }

    // StepFun
    const plan = d.plan_name || 'Unknown';
    const fhReset = d.five_hour_usage_reset_time || null;
    const wkReset = d.weekly_usage_reset_time || null;
    const fhLeft = d.five_hour_usage_left_rate != null ? Math.round(d.five_hour_usage_left_rate * 100) : '--';
    const wkLeft = d.weekly_usage_left_rate != null ? Math.round(d.weekly_usage_left_rate * 100) : '--';

    box.add_child(new St.Label({
        text: `Plan: ${plan}`,
        style: 'font-size: 11px; color: #3584e4; font-weight: bold; padding: 2px 12px 2px 32px;',
    }));
    box.add_child(new St.Label({
        text: `5h: ${fhLeft}%  (reset ${_fmtReset(fhReset)})`,
        style: 'font-size: 11px; color: #999; padding: 2px 12px 2px 32px;',
    }));
    box.add_child(new St.Label({
        text: `Week: ${wkLeft}%  (reset ${_fmtReset(wkReset)})`,
        style: 'font-size: 11px; color: #999; padding: 2px 12px 2px 32px;',
    }));

    // Progress bar
    const pct = Math.round(provider.remaining_percent || 0);
    const w = 200;
    const fw = Math.max(0, Math.round(pct / 100 * w));
    let cls = 'high';
    if (pct < 20) cls = 'low';
    else if (pct < 50) cls = 'medium';

    const barBox = new St.BoxLayout({ vertical: true, style: 'padding: 6px 12px 8px 32px;' });
    const track = new St.BoxLayout({ style_class: 'codex-bar-bar-bg', style: `width: ${w}px;` });
    track.add_child(new St.BoxLayout({ style: `width: ${fw}px;`, style_class: `codex-bar-bar-fill ${cls}` }));
    barBox.add_child(track);
    barBox.add_child(new St.Label({ text: `${pct}%`, x_expand: true, style_class: `codex-bar-percent ${cls}` }));
    box.add_child(barBox);
}

function _addRefreshBtn(menu) {
    const item = new PopupMenu.PopupBaseMenuItem({ reactive: false });
    const row = new St.BoxLayout({ style: 'padding: 6px 16px;' });
    row.add_child(new St.Label({ text: '', x_expand: true }));

    const btn = new St.Button({ label: '\u21BB Refresh', style_class: 'codex-bar-button' });
    btn.connect('button-press-event', () => {
        try {
            GLib.spawn_command_line_async('codex-bar-cli fetch');
        } catch (e) {
            log('[codex-bar] Failed to trigger refresh: ' + e.message);
        }
        // Defer rebuild to let CLI write new status.json
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 2000, () => {
            rebuildMenu(menu);
            return GLib.SOURCE_REMOVE;
        });
        return Clutter.EVENT_STOP;
    });
    row.add_child(btn);
    item.add_child(row);
    menu.addMenuItem(item);
}

function _fmtReset(ts) {
    if (!ts) return '--';
    try {
        const d = new Date(ts).getTime() - Date.now();
        if (d < 0) return 'now';
        const h = Math.floor(d / 3600000);
        const m = Math.floor((d % 3600000) / 60000);
        if (h > 24) return `${Math.floor(h / 24)}d`;
        if (h > 0) return `${h}h${m > 0 ? m + 'm' : ''}`;
        return `${m}m`;
    } catch (e) { return '--'; }
}
