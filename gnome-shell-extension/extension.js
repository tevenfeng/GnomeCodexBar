import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as StatusReader from './statusReader.js';
import {ConfigManager} from './configManager.js';

const P = { deepseek: 'DeepSeek', stepfun: 'StepFun', opencodego: 'OpenCode Go' };

// Theme-aware color palette
const C = {
    dark: {
        text: '#eeeeee',
        sub: '#cccccc',
        muted: '#999999',
        dim: '#888888',
        faint: '#666666',
        val: '#bbbbbb',
        accent: '#3584e4',
        err: '#cc3333',
        warn: '#e6a817',
        ok: '#33cc66',
        border: 'rgba(255,255,255,0.06)',
        borderSel: 'rgba(53,132,228,0.25)',
        bgCard: 'rgba(255,255,255,0.05)',
        bgCardSel: 'rgba(53,132,228,0.12)',
        bgBar: 'rgba(255,255,255,0.12)',
        borderFt: 'rgba(255,255,255,0.08)',
        divider: 'rgba(255,255,255,0.08)',
        btnBg: 'rgba(255,255,255,0.08)',
        btnBgHover: 'rgba(255,255,255,0.15)',
    },
    light: {
        text: '#1a1a1a',
        sub: '#333333',
        muted: '#666666',
        dim: '#555555',
        faint: '#999999',
        val: '#222222',
        accent: '#1a6dd4',
        err: '#cc3333',
        warn: '#c48a00',
        ok: '#2ea043',
        border: 'rgba(0,0,0,0.1)',
        borderSel: 'rgba(26,109,212,0.3)',
        bgCard: 'rgba(0,0,0,0.04)',
        bgCardSel: 'rgba(26,109,212,0.08)',
        bgBar: 'rgba(0,0,0,0.1)',
        borderFt: 'rgba(0,0,0,0.08)',
        divider: 'rgba(0,0,0,0.08)',
        btnBg: 'rgba(0,0,0,0.06)',
        btnBgHover: 'rgba(0,0,0,0.12)',
    },
};

export default class CodexBarExtension extends Extension {
    enable() {
        this._ifaceSettings = Gio.Settings.new('org.gnome.desktop.interface');
        const onTheme = () => {
            if (this._popup?.visible) {
                this._popup.style = this._isLight() ? 'background-color:#fafafa;' : 'background-color:#2a2a2a;';
                this._popup.destroy_all_children();
                this._show();
            }
            this._updatePanel();
        };
        this._themeSig = this._ifaceSettings.connect('changed::color-scheme', onTheme);
        this._gtkThemeSig = this._ifaceSettings.connect('changed::gtk-theme', onTheme);

        this._btn = new St.Bin({ style_class: 'panel-button', reactive: true, can_focus: true, track_hover: true });
        const bb = new St.BoxLayout({});
        this._sn = new St.Label({ text:'', y_align:Clutter.ActorAlign.CENTER, style:'font-size:10px; margin-right:4px;' });
        this._ic = new St.Label({ text:'\u25CF', y_align:Clutter.ActorAlign.CENTER, style:'font-size:10px; margin-right:4px;' });
        this._lb = new St.Label({ text:'--', y_align:Clutter.ActorAlign.CENTER, style_class:'codex-bar-panel-label' });
        bb.add_child(this._sn); bb.add_child(this._ic); bb.add_child(this._lb);
        this._btn.set_child(bb);
        Main.panel._rightBox.insert_child_at_index(this._btn, 0);

        this._popupBackdrop = new St.Widget({
            reactive: true,
            visible: false,
            style: 'background-color: transparent;',
        });
        this._popupBackdrop.connect('button-press-event', (actor, ev) => {
            if (ev.get_button() === 1) {
                this._hidePopup();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        this._popup = new St.BoxLayout({ vertical:true, style_class:'codex-bar-popup', reactive:true, style:'background-color:#2a2a2a;' });
        this._popup.hide();
        Main.layoutManager.addChrome(this._popupBackdrop, { affectsInputRegion: true });
        Main.layoutManager.addChrome(this._popup, { affectsInputRegion: true });

        this._btn.connect('button-press-event', (a, ev) => {
            if (ev.get_button() !== 1) return Clutter.EVENT_PROPAGATE;
            this._popup.visible ? this._hidePopup() : this._show();
            return Clutter.EVENT_STOP;
        });

        // Close popup on click outside (shell-internal + captured before actors)
        this._captureSig = global.stage.connect('captured-event', (stage, ev) => {
            if (!this._popup || !this._popup.visible) return Clutter.EVENT_PROPAGATE;
            if (ev.type() === Clutter.EventType.BUTTON_PRESS && ev.get_button() === 1) {
                const [x, y] = ev.get_coords();
                const [px, py] = this._popup.get_transformed_position();
                const [pw, ph] = this._popup.get_transformed_size();
                const [bx, by] = this._btn.get_transformed_position();
                const [bw, bh] = this._btn.get_transformed_size();
                const source = ev.get_source?.();
                if (source === this._popupBackdrop) return Clutter.EVENT_PROPAGATE;
                if (x < px || x > px + pw || y < py || y > py + ph) {
                    if (x < bx || x > bx + bw || y < by || y > by + bh) {
                        this._hidePopup();
                        return Clutter.EVENT_STOP;
                    }
                }
            } else if (ev.type() === Clutter.EventType.KEY_PRESS) {
                const sym = ev.get_key_symbol();
                if (sym === Clutter.KEY_Escape) {
                    this._hidePopup();
                    return Clutter.EVENT_STOP;
                }
            }
            return Clutter.EVENT_PROPAGATE;
        });

        // Close popup when a window gains focus
        this._focusSig = global.display.connect('notify::focus-window', () => {
            if (!this._popup?.visible) return;
            if (!global.display.focus_window) return;
            GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                if (this._popup?.visible) this._hidePopup();
                return GLib.SOURCE_REMOVE;
            });
        });

        // Close popup when overview is opened
        this._overviewSig = Main.overview.connect('showing', () => {
            if (this._popup?.visible) this._hidePopup();
        });

        this._updatePanel();

        // Read refresh interval from config.toml (primary), fallback to GSettings
        this._cfg = new ConfigManager();
        let iv = this._cfg.getRefreshInterval();
        if (iv === null) iv = this.getSettings().get_int('refresh-interval');
        this._startTimer(iv);

        // Monitor config.toml for changes
        this._cfg.monitor(() => {
            const newIv = this._cfg.getRefreshInterval() ?? this.getSettings().get_int('refresh-interval');
            if (newIv !== this._currIv) this._startTimer(newIv);
            this._updatePanel();
        });

        this._m = StatusReader.monitorStatus(() => this._updatePanel());
    }

    disable() {
        if (this._t) GLib.Source.remove(this._t);
        if (this._m) this._m.cancel();
        if (this._cfg) { this._cfg.destroy(); this._cfg = null; }
        if (this._themeSig) { this._ifaceSettings.disconnect(this._themeSig); this._themeSig = null; }
        if (this._gtkThemeSig) { this._ifaceSettings.disconnect(this._gtkThemeSig); this._gtkThemeSig = null; }
        if (this._captureSig) { global.stage.disconnect(this._captureSig); this._captureSig = null; }
        if (this._focusSig) { global.display.disconnect(this._focusSig); this._focusSig = null; }
        if (this._overviewSig) { Main.overview.disconnect(this._overviewSig); this._overviewSig = null; }
        [this._popup, this._popupBackdrop, this._btn].forEach(w => w?.destroy());
        this._btn = this._popup = this._popupBackdrop = null;
    }

    /** Returns true if the system is using a light theme */
    _isLight() {
        const colorScheme = this._ifaceSettings?.get_enum('color-scheme') ?? 0;
        if (colorScheme === 2) return true;
        if (colorScheme === 1) return false;
        try {
            const theme = this._ifaceSettings?.get_string('gtk-theme') || '';
            if (/light/i.test(theme)) return true;
            if (/dark/i.test(theme)) return false;
        } catch (e) {}
        try {
            const node = Main.panel._centerBox.get_theme_node();
            const [ok, color] = node.lookup_color('background-color', false);
            if (ok) return color.luminance >= 0.5;
        } catch (e) {}
        return false;
    }

    _c() { return this._isLight() ? C.light : C.dark; }

    _statusColor(v) {
        const c = this._c();
        if (v < 20) return c.err;
        if (v < 50) return c.warn;
        return c.ok;
    }

    _shortLabel(pid) {
        if (pid === 'deepseek') return 'DS';
        if (pid === 'stepfun') return 'SF';
        if (pid === 'opencodego') return 'OCG';
        return (pid || '--').slice(0, 3).toUpperCase();
    }

    /** Format a relative time string for "Updated X前" */
    _ago(ts) {
        if (!ts) return 'just now';
        try {
            const diff = Date.now() - new Date(ts).getTime();
            if (diff < 60000) return 'just now';
            const m = Math.floor(diff / 60000);
            if (m < 60) return `${m}m ago`;
            const h = Math.floor(m / 60);
            if (h < 24) return `${h}h ago`;
            return `${Math.floor(h / 24)}d ago`;
        } catch (e) { return 'just now'; }
    }

    /** Format a relative time string for "Resets in X" */
    _resetIn(ts) {
        if (!ts) return null;
        try {
            const d = new Date(ts).getTime() - Date.now();
            if (d < 0) return 'now';
            const h = Math.floor(d / 3600000), m = Math.floor((d % 3600000) / 60000);
            if (h > 24) return `${Math.floor(h / 24)}d ${h % 24 > 0 ? (h % 24) + 'h' : ''}`;
            if (h > 0) return `${h}h${m > 0 ? m + 'm' : ''}`;
            return `${m}m`;
        } catch (e) { return null; }
    }

    _startTimer(seconds) {
        if (this._t) { GLib.Source.remove(this._t); this._t = null; }
        this._currIv = seconds;
        this._t = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => { this._updatePanel(); return GLib.SOURCE_CONTINUE; });
    }

    _updatePanel() {
        if (!this._btn) return;
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        const c = this._c();
        if (!s?.providers) { this._sn.text=''; this._ic.style=`font-size:10px; margin-right:4px; color:${c.err};`; this._lb.text='--'; return; }
        // Filter by enabled providers
        const enabled = this._cfg?.getProviderEnabledMap() || { deepseek: true, stepfun: true };
        const active = s.providers.filter(p => enabled[p.provider_id] !== false);
        if (active.length === 0) { this._sn.text=''; this._ic.style=`font-size:10px; margin-right:4px; color:${c.faint};`; this._lb.text='--'; return; }
        const pr = active.find(x => x.provider_id === sel) || active[0];
        this._showBtn(pr, sel);
    }

    _hidePopup() {
        if (this._popup?.visible) this._popup.hide();
        if (this._popupBackdrop?.visible) this._popupBackdrop.hide();
    }

    _updatePopupBackdropGeometry() {
        if (!this._popupBackdrop) return;
        const m = Main.layoutManager.primaryMonitor;
        const [, panelY] = Main.panel.get_transformed_position();
        const [, panelH] = Main.panel.get_transformed_size();
        const y = Math.max(m.y, panelY + panelH);
        const height = Math.max(1, m.y + m.height - y);
        this._popupBackdrop.set_position(m.x, y);
        this._popupBackdrop.set_size(m.width, height);
    }

    _showBtn(pr, sel) {
        const c = this._c();
        const pid = pr?.provider_id || sel;
        this._sn.text = this._shortLabel(pid);
        if (pr.error) { this._ic.style=`font-size:10px; margin-right:4px; color:${c.err};`; this._lb.text='ERR'; return; }
        if (pid === 'deepseek') {
            const d=pr.details||{}, t=+d.total_balance||0, cu=d.currency==='USD'?'$':'\u00A5';
            let tx=`${cu}${t.toFixed(2)}`; if(tx.length>10) tx=`${cu}${Math.round(t)}`;
            this._ic.style=`font-size:10px; margin-right:4px; color:${c.accent};`; this._lb.text=tx;
        } else {
            const v=Math.round(pr.remaining_percent||0);
            const cl = this._statusColor(v);
            this._ic.style=`font-size:10px; margin-right:4px; color:${cl};`; this._lb.text=`${v}%`;
        }
    }

    _show() {
        this._popup.destroy_all_children();
        const light = this._isLight();
        if (light) {
            this._popup.add_style_class_name('codex-bar-light');
            this._popup.style = 'background-color:#fafafa;';
        } else {
            this._popup.remove_style_class_name('codex-bar-light');
            this._popup.style = 'background-color:#2a2a2a;';
        }
        this._build();
        const [bx,by] = this._btn.get_transformed_position();
        const [bw,bh] = this._btn.get_transformed_size();
        const m = Main.layoutManager.primaryMonitor;
        let pw = this._popup.get_preferred_width(-1)[1]; if (pw<100) pw=310;
        let px = bx + (bw-pw)/2;
        if (px < m.x) px = m.x+8;
        if (px+pw > m.x+m.width) px = m.x+m.width-pw-8;
        this._popup.set_position(Math.round(px), Math.round(by+bh+4));
        this._popup.show();
        this._updatePopupBackdropGeometry();
        this._popupBackdrop?.show();
    }

    _build() {
        const c = this._c();
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        const r = new St.BoxLayout({ vertical:true, style_class:'codex-bar-container' });

        // Title row: "Codex Bar" + refresh button
        const titleRow = new St.BoxLayout({ style: 'margin-bottom:8px;' });
        titleRow.add_child(new St.Label({ text:'Codex Bar', style_class:'codex-bar-title', x_expand:true }));
        const refBtn = new St.Button({ label:'\u21BB', style_class:'codex-bar-refresh-btn', style:`color:${c.faint}; font-size:14px; padding:2px 6px; border-radius:4px;` });
        refBtn.connect('button-press-event', () => {
            try { GLib.spawn_command_line_async('codex-bar-cli fetch'); } catch(e) {}
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 2000, () => {
                this._updatePanel();
                if (this._popup?.visible) this._show();
                return GLib.SOURCE_REMOVE;
            });
            return Clutter.EVENT_STOP;
        });
        titleRow.add_child(refBtn);
        r.add_child(titleRow);

        if (!s?.providers) {
            r.add_child(new St.Label({ text:'No data.\nRun "codex-bar-cli daemon" first.', style:`font-size:12px; color:${c.muted}; padding:8px 0;` }));
        } else {
            const enabled = this._cfg?.getProviderEnabledMap() || { deepseek: true, stepfun: true };
            const providers = s.providers.filter(p => enabled[p.provider_id] !== false);
            if (providers.length === 0) {
                r.add_child(new St.Label({ text:'No providers enabled.\nEnable providers in extension settings.', style:`font-size:12px; color:${c.muted}; padding:8px 0;` }));
            } else {
                for (let i = 0; i < providers.length; i++) {
                    if (i > 0) {
                        r.add_child(new St.Widget({ style: `height:1px; background-color:${c.divider}; margin:8px 0;` }));
                    }
                    this._addProviderSection(r, providers[i], sel, s.updated_at);
                }
            }
        }
        this._popup.add_child(r);
    }

    _addProviderSection(r, pr, sel, updatedAt) {
        const c = this._c();
        const pid = pr.provider_id, nm = P[pid] || pid, is = pid === sel;
        const d = pr.details || {};

        // Background tint for selected provider
        const secStyle = is
            ? `padding:10px 12px; border-radius:8px; background:${c.bgCardSel};`
            : `padding:10px 12px; border-radius:8px; background:${c.bgCard};`;
        const sec = new St.BoxLayout({ vertical: true, style: secStyle });

        // Header: clickable to switch provider
        const hdr = new St.Button({ reactive: true, can_focus: true, track_hover: true, x_expand: true, style: 'background:transparent; border:none; padding:0;' });
        const hdrContent = new St.BoxLayout({ vertical: true, x_expand: true });

        // Row 1: Name + Plan tag
        const nameRow = new St.BoxLayout({ x_expand: true });
        nameRow.add_child(new St.Label({ text: nm, x_expand: true, style: is ? `font-size:16px; font-weight:600; color:${c.accent};` : `font-size:16px; font-weight:600; color:${c.sub};` }));
        // Optional plan tag for quota-style providers
        if (d.plan_name) {
            nameRow.add_child(new St.Label({ text: d.plan_name, style: `font-size:12px; color:${c.muted};` }));
        }
        hdrContent.add_child(nameRow);

        // Row 2: Updated time
        hdrContent.add_child(new St.Label({ text: `Updated ${this._ago(updatedAt)}`, style: `font-size:12px; color:${c.faint}; margin-top:2px;` }));

        hdr.set_child(hdrContent);
        hdr.connect('button-press-event', () => {
            StatusReader.writeSelectedProvider(pid);
            this._updatePanel();
            this._show();
            return Clutter.EVENT_STOP;
        });
        sec.add_child(hdr);

        // Divider between header and bars
        sec.add_child(new St.Widget({ style: `height:1px; background-color:${c.divider}; margin:8px 0;` }));

        // Details
        if (pr.error) {
            sec.add_child(new St.Label({ text: `Error: ${pr.error}`, style: `font-size:12px; color:${c.err};` }));
        } else {
            this._addDetails(sec, pr);
        }

        r.add_child(sec);
    }

    _addDetails(sec, pr) {
        const c = this._c();
        const pid = pr.provider_id, d = pr.details || {};

        if (pid === 'deepseek') {
            // Balance bar
            const hasBalance = (+d.total_balance || 0) > 0;
            const pct = hasBalance ? 100 : 0;
            this._addBar(sec, 'Balance', pct, null);

            // Balance detail text
            const cu = d.currency || 'CNY', s = cu === 'USD' ? '$' : '\u00A5';
            const total = +d.total_balance || 0;
            const paid = +d.topped_up_balance || 0;
            const granted = +d.granted_balance || 0;
            sec.add_child(new St.Label({
                text: `${s}${total.toFixed(2)} (Paid: ${s}${paid.toFixed(2)} / Granted: ${s}${granted.toFixed(2)})`,
                style: `font-size:11px; color:${c.muted}; margin-top:4px;`
            }));
        } else {
            // Quota providers: 5h Window + Weekly Window + optional Monthly Window
            if (d.five_hour_usage_left_rate != null) {
                const pct = Math.round(d.five_hour_usage_left_rate * 100);
                this._addBar(sec, '5h Window', pct, d.five_hour_usage_reset_time);
            }
            if (d.weekly_usage_left_rate != null) {
                const pct = Math.round(d.weekly_usage_left_rate * 100);
                this._addBar(sec, 'Weekly Window', pct, d.weekly_usage_reset_time);
            }
            if (d.monthly_usage_left_rate != null) {
                const pct = Math.round(d.monthly_usage_left_rate * 100);
                this._addBar(sec, 'Monthly Window', pct, d.monthly_usage_reset_time);
            }
        }
    }

    _addBar(parent, title, pct, resetTime) {
        const c = this._c();
        const box = new St.BoxLayout({ vertical: true, style: 'margin-top:10px;' });

        // Title row
        box.add_child(new St.Label({ text: title, style: `font-size:14px; font-weight:600; color:${c.text};` }));

        // Progress bar — two-segment St.BoxLayout with CSS widths
        const barW = 250, barH = 8, fillW = Math.max(0, Math.round(pct / 100 * barW));
        let cl = 'high'; if (pct < 20) cl = 'low'; else if (pct < 50) cl = 'medium';
        const barBox = new St.BoxLayout({ style: `width:${barW}px; height:${barH}px; margin:6px 0 4px 0; spacing:0; border-radius:4px; overflow:hidden;` });
        const fillSeg = new St.Widget({ style_class: `codex-bar-bar-fill ${cl}` });
        fillSeg.set_style(`width:${fillW}px; height:${barH}px;`);
        const restSeg = new St.Widget({ style_class: 'codex-bar-bar-bg' });
        restSeg.set_style(`width:${barW - fillW}px; height:${barH}px;`);
        barBox.add_child(fillSeg);
        barBox.add_child(restSeg);
        box.add_child(barBox);

        // Info row: "X% left" left + "Resets in X" right
        const infoRow = new St.BoxLayout({ x_expand: true });
        infoRow.add_child(new St.Label({ text: `${pct}% left`, style: `font-size:12px; color:${c.dim};` }));
        infoRow.add_child(new St.Widget({ x_expand: true }));
        const resetStr = this._resetIn(resetTime);
        if (resetStr) {
            infoRow.add_child(new St.Label({ text: `Resets in ${resetStr}`, style: `font-size:12px; color:${c.faint};` }));
        }
        box.add_child(infoRow);

        parent.add_child(box);
    }
}
