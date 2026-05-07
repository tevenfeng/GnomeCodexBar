import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as StatusReader from './statusReader.js';

const P = { deepseek: 'DeepSeek', stepfun: 'StepFun' };

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
        btnBg: 'rgba(0,0,0,0.06)',
        btnBgHover: 'rgba(0,0,0,0.12)',
    },
};

export default class CodexBarExtension extends Extension {
    enable() {
        this._ifaceSettings = Gio.Settings.new('org.gnome.desktop.interface');
        const onTheme = () => {
            if (this._popup?.visible) {
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

        this._popup = new St.BoxLayout({ vertical:true, style_class:'codex-bar-popup', reactive:true });
        this._popup.hide();
        Main.layoutManager.addChrome(this._popup, { affectsInputRegion: true });

        this._btn.connect('button-press-event', (a, ev) => {
            if (ev.get_button() !== 1) return Clutter.EVENT_PROPAGATE;
            this._popup.visible ? this._popup.hide() : this._show();
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
                if (x < px || x > px + pw || y < py || y > py + ph) {
                    if (x < bx || x > bx + bw || y < by || y > by + bh) {
                        this._popup.hide();
                        return Clutter.EVENT_STOP;
                    }
                }
            } else if (ev.type() === Clutter.EventType.KEY_PRESS) {
                const sym = ev.get_key_symbol();
                if (sym === Clutter.KEY_Escape) {
                    this._popup.hide();
                    return Clutter.EVENT_STOP;
                }
            }
            return Clutter.EVENT_PROPAGATE;
        });

        // Close popup when a window gains focus
        this._focusSig = global.display.connect('notify::focus-window', () => {
            if (this._popup?.visible) {
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    if (this._popup?.visible) this._popup.hide();
                    return GLib.SOURCE_REMOVE;
                });
            }
        });

        // Close popup when overview is opened
        this._overviewSig = Main.overview.connect('showing', () => {
            if (this._popup?.visible) this._popup.hide();
        });

        this._updatePanel();
        const iv = this.getSettings().get_int('refresh-interval');
        this._t = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, iv, () => { this._updatePanel(); return GLib.SOURCE_CONTINUE; });
        this._m = StatusReader.monitorStatus(() => this._updatePanel());
    }

    disable() {
        if (this._t) GLib.Source.remove(this._t);
        if (this._m) this._m.cancel();
        if (this._themeSig) { this._ifaceSettings.disconnect(this._themeSig); this._themeSig = null; }
        if (this._gtkThemeSig) { this._ifaceSettings.disconnect(this._gtkThemeSig); this._gtkThemeSig = null; }
        if (this._captureSig) { global.stage.disconnect(this._captureSig); this._captureSig = null; }
        if (this._focusSig) { global.display.disconnect(this._focusSig); this._focusSig = null; }
        if (this._overviewSig) { Main.overview.disconnect(this._overviewSig); this._overviewSig = null; }
        [this._popup, this._btn].forEach(w => w?.destroy());
        this._btn = this._popup = null;
    }

    /** Returns true if the system is using a light theme */
    _isLight() {
        const colorScheme = this._ifaceSettings?.get_enum('color-scheme') ?? 0;
        if (colorScheme === 2) return true;   // 2 = prefer-light
        if (colorScheme === 1) return false;  // 1 = prefer-dark
        // 0 = default/no preference
        // Fallback 1: gtk-theme name
        try {
            const theme = this._ifaceSettings?.get_string('gtk-theme') || '';
            if (/light/i.test(theme)) return true;
            if (/dark/i.test(theme)) return false;
        } catch (e) {}
        // Fallback 2: panel background luminance
        try {
            const node = Main.panel._centerBox.get_theme_node();
            const [ok, color] = node.lookup_color('background-color', false);
            if (ok) { return color.luminance >= 0.5; }
        } catch (e) {}
        return false; // assume dark
    }

    /** Get the color palette for current theme */
    _c() {
        return this._isLight() ? C.light : C.dark;
    }

    /** Status color based on percentage and theme */
    _statusColor(v) {
        const c = this._c();
        if (v < 20) return c.err;
        if (v < 50) return c.warn;
        return c.ok;
    }

    _updatePanel() {
        if (!this._btn) return;
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        const c = this._c();
        if (!s?.providers) { this._sn.text=''; this._ic.style=`font-size:10px; margin-right:4px; color:${c.err};`; this._lb.text='--'; return; }
        const pr = s.providers.find(x => x.provider_id === sel) || s.providers[0];
        this._showBtn(pr, sel);
    }

    _showBtn(pr, sel) {
        const c = this._c();
        this._sn.text = sel === 'deepseek' ? 'DS' : 'SF';
        if (pr.error) { this._ic.style=`font-size:10px; margin-right:4px; color:${c.err};`; this._lb.text='ERR'; return; }
        if (sel === 'deepseek') {
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
        // Apply theme class to popup
        const light = this._isLight();
        if (light) {
            this._popup.add_style_class_name('codex-bar-light');
        } else {
            this._popup.remove_style_class_name('codex-bar-light');
        }
        this._build();
        const [bx,by] = this._btn.get_transformed_position();
        const [bw,bh] = this._btn.get_transformed_size();
        const m = Main.layoutManager.primaryMonitor;
        let pw = this._popup.get_preferred_width(-1)[1]; if (pw<100) pw=340;
        let px = bx + (bw-pw)/2;
        if (px < m.x) px = m.x+8;
        if (px+pw > m.x+m.width) px = m.x+m.width-pw-8;
        this._popup.set_position(Math.round(px), Math.round(by+bh+4));
        this._popup.show();
    }

    _build() {
        const c = this._c();
        const r = new St.BoxLayout({ vertical:true, style_class:'codex-bar-container' });
        r.add_child(new St.Label({ text:'Codex Bar', style_class:'codex-bar-title', x_expand:true }));
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        if (!s?.providers) {
            r.add_child(new St.Label({ text:'No data.\nRun "codex-bar-cli daemon" first.', style:`font-size:11px; color:${c.muted}; padding:12px;` }));
        } else {
            for (const pr of s.providers) this._addProv(r, pr, sel);
            this._addFt(r);
        }
        this._popup.add_child(r);
    }

    _addProv(r, pr, sel) {
        const c = this._c();
        const pid=pr.provider_id, nm=P[pid]||pid, is=pid===sel;
        const cardStyle = is
            ? `margin:4px 12px; padding:10px 12px; border:1px solid ${c.borderSel}; border-radius:8px; background:${c.bgCardSel};`
            : `margin:4px 12px; padding:10px 12px; border:1px solid ${c.border}; border-radius:8px; background:${c.bgCard};`;
        const sec = new St.BoxLayout({ vertical:true, style:cardStyle });
        const hdr = new St.Button({ reactive:true, can_focus:true, track_hover:true, style:'background:transparent; border:none; padding:0;' });
        const hb = new St.BoxLayout({ style:'x-align:start;' });
        hb.add_child(new St.Label({ text:is?'\u25C9':'\u25CB', style:is?`color:${c.accent}; width:22px; font-size:15px;`:`color:${c.faint}; width:22px; font-size:15px;` }));
        hb.add_child(new St.Label({ text:nm, style:is?`color:${c.accent}; font-size:13px; font-weight:600;`:`color:${c.sub}; font-size:13px; font-weight:600;` }));
        hdr.set_child(hb);
        hdr.connect('button-press-event', () => { StatusReader.writeSelectedProvider(pid); this._updatePanel(); this._popup.destroy_all_children(); this._build(); this._popup.show(); return Clutter.EVENT_STOP; });
        sec.add_child(hdr);
        if (pr.error) sec.add_child(new St.Label({ text:`Error: ${pr.error}`, style:`font-size:11px; color:${c.err}; padding:4px 0 0 22px;` }));
        else this._dtl(sec, pr);
        r.add_child(sec);
    }

    _dtl(sec, pr) {
        const c = this._c();
        const pid=pr.provider_id, d=pr.details||{};
        if (pid==='deepseek') {
            const cu=d.currency||'CNY', s=cu==='USD'?'$':'\u00A5';
            for (const [l,k] of [['Balance','total_balance'],['Granted','granted_balance'],['Topped Up','topped_up_balance']]) {
                const v=+d[k]||0;
                const row=new St.BoxLayout({ style:'padding:2px 0 2px 24px;' });
                row.add_child(new St.Label({ text:l+': ', style:`font-size:11px; color:${c.dim};` }));
                row.add_child(new St.Label({ text:`${s}${v.toFixed(2)}`, style:`font-size:11px; color:${c.val}; font-family:monospace;` }));
                sec.add_child(row);
            }
        } else {
            const dn=d.plan_name||'Unknown';
            sec.add_child(new St.Label({ text:`Plan: ${dn}`, style:`font-size:11px; color:${c.accent}; font-weight:bold; padding:2px 0 2px 24px;` }));
            if (d.five_hour_usage_left_rate!=null) sec.add_child(this._barP(d.five_hour_usage_left_rate, '5h', d.five_hour_usage_reset_time));
            if (d.weekly_usage_left_rate!=null) sec.add_child(this._barP(d.weekly_usage_left_rate, 'Week', d.weekly_usage_reset_time));
        }
    }

    _barP(rate, label, resetTime) {
        const c = this._c(), pct = Math.round(rate * 100), w2 = 140, f = Math.max(0, Math.round(rate * w2));
        let cl = 'high'; if (pct < 20) cl = 'low'; else if (pct < 50) cl = 'medium';
        const row = new St.BoxLayout({ style: 'padding:3px 0 3px 24px;' });
        row.add_child(new St.Label({ text: `${label}:`, style: `font-size:10px; color:${c.dim}; margin-right:8px; min-width:36px;` }));
        const segs = new St.BoxLayout({ style: `width:${w2}px; height:6px; spacing:0;` });
        segs.add_child(new St.Widget({ style: `width:${f}px; height:6px; border-radius:3px 0 0 3px;`, style_class: `codex-bar-bar-fill ${cl}` }));
        segs.add_child(new St.Widget({ style: `width:${w2 - f}px; height:6px; border-radius:0 3px 3px 0; background-color:${c.bgBar};` }));
        const barWrap = new St.BoxLayout({ style: 'margin-right:8px;' });
        barWrap.add_child(segs);
        row.add_child(barWrap);
        row.add_child(new St.Label({ text: `${pct}%`, style_class: `codex-bar-percent ${cl}`, style: 'margin-right:8px;' }));
        row.add_child(new St.Label({ text: `reset ${this._r(resetTime)}`, style: `font-size:9px; color:${c.faint};` }));
        return row;
    }

    _addFt(r) {
        const c = this._c();
        const f=new St.BoxLayout({ style:`padding:8px 12px; border-top:1px solid ${c.borderFt}; margin-top:4px;` });
        const st=StatusReader.readStatus();
        f.add_child(new St.Label({ text:st?.updated_at?new Date(st.updated_at).toLocaleTimeString():'Never', style:`font-size:10px; color:${c.faint};` }));
        f.add_child(new St.Widget({ x_expand:true }));
        const btn=new St.Button({ label:'\u21BB Refresh', style_class:'codex-bar-button' });
        btn.connect('button-press-event', () => { try{GLib.spawn_command_line_async('codex-bar-cli fetch');}catch(e){} GLib.timeout_add(GLib.PRIORITY_DEFAULT,2000,()=>{this._updatePanel(); if(this._popup.visible){this._popup.destroy_all_children();this._build();} return GLib.SOURCE_REMOVE;}); return Clutter.EVENT_STOP; });
        f.add_child(btn);
        r.add_child(f);
    }

    _r(ts) { if(!ts)return'--'; try{const d=new Date(ts)-Date.now(); if(d<0)return'now'; const h=Math.floor(d/36e5),m=Math.floor((d%36e5)/6e4); if(h>24)return`${Math.floor(h/24)}d`; if(h>0)return`${h}h${m>0?m+'m':''}`; return`${m}m`;}catch(e){return'--';} }
}
