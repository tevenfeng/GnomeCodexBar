import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as StatusReader from './statusReader.js';

const P = { deepseek: 'DeepSeek', stepfun: 'StepFun' };

export default class CodexBarExtension extends Extension {
    enable() {
        this._btn = new St.Bin({ style_class: 'panel-button', reactive: true, can_focus: true, track_hover: true });
        const bb = new St.BoxLayout({});
        this._sn = new St.Label({ text:'', y_align:Clutter.ActorAlign.CENTER, style:'font-size:10px; margin-right:4px; color:#888;' });
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

        this._updatePanel();
        const iv = this.getSettings().get_int('refresh-interval');
        this._t = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, iv, () => { this._updatePanel(); return GLib.SOURCE_CONTINUE; });
        this._m = StatusReader.monitorStatus(() => this._updatePanel());
    }

    disable() {
        if (this._t) GLib.Source.remove(this._t);
        if (this._m) this._m.cancel();
        [this._popup, this._btn].forEach(w => w?.destroy());
        this._btn = this._popup = null;
    }

    _updatePanel() {
        if (!this._btn) return;
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        if (!s?.providers) { this._sn.text=''; this._ic.style='font-size:10px; margin-right:4px; color:#cc3333;'; this._lb.text='--'; return; }
        const pr = s.providers.find(x => x.provider_id === sel) || s.providers[0];
        this._showBtn(pr, sel);
    }

    _showBtn(pr, sel) {
        this._sn.text = sel === 'deepseek' ? 'DS' : 'SF';
        if (pr.error) { this._ic.style='font-size:10px; margin-right:4px; color:#cc3333;'; this._lb.text='ERR'; return; }
        if (sel === 'deepseek') {
            const d=pr.details||{}, t=+d.total_balance||0, c=d.currency==='USD'?'$':'\u00A5';
            let tx=`${c}${t.toFixed(2)}`; if(tx.length>10) tx=`${c}${Math.round(t)}`;
            this._ic.style='font-size:10px; margin-right:4px; color:#3584e4;'; this._lb.text=tx;
        } else {
            const v=Math.round(pr.remaining_percent||0);
            let cl='#33cc66'; if(v<20) cl='#cc3333'; else if(v<50) cl='#e6a817';
            this._ic.style=`font-size:10px; margin-right:4px; color:${cl};`; this._lb.text=`${v}%`;
        }
    }

    _show() {
        this._popup.destroy_all_children();
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
        const r = new St.BoxLayout({ vertical:true, style_class:'codex-bar-container' });
        r.add_child(new St.Label({ text:'Codex Bar', style_class:'codex-bar-title', x_expand:true }));
        const s = StatusReader.readStatus(), sel = StatusReader.readSelectedProvider();
        if (!s?.providers) {
            r.add_child(new St.Label({ text:'No data.\nRun "codex-bar-cli daemon" first.', style:'font-size:11px; color:#999; padding:12px;' }));
        } else {
            for (const pr of s.providers) this._addProv(r, pr, sel);
            this._addFt(r);
        }
        this._popup.add_child(r);
    }

    _addProv(r, pr, sel) {
        const pid=pr.provider_id, nm=P[pid]||pid, is=pid===sel;
        const sec = new St.BoxLayout({ vertical:true, style:is?'margin:4px 12px; padding:10px 12px; border:1px solid rgba(53,132,228,0.25); border-radius:8px; background:rgba(53,132,228,0.12);':'margin:4px 12px; padding:10px 12px; border:1px solid rgba(255,255,255,0.06); border-radius:8px; background:rgba(255,255,255,0.05);' });
        const hdr = new St.Button({ reactive:true, can_focus:true, track_hover:true, x_expand:true, style:'background:transparent; border:none; padding:0;' });
        const hb = new St.BoxLayout({});
        hb.add_child(new St.Label({ text:is?'\u25C9':'\u25CB', style:is?'color:#3584e4; width:22px; font-size:15px;':'color:#666; width:22px; font-size:15px;' }));
        hb.add_child(new St.Label({ text:nm, style:is?'color:#3584e4; font-size:13px; font-weight:600;':'color:#ccc; font-size:13px; font-weight:600;' }));
        hdr.set_child(hb);
        hdr.connect('button-press-event', () => { StatusReader.writeSelectedProvider(pid); this._updatePanel(); this._popup.destroy_all_children(); this._build(); this._popup.show(); return Clutter.EVENT_STOP; });
        sec.add_child(hdr);
        if (pr.error) sec.add_child(new St.Label({ text:`Error: ${pr.error}`, style:'font-size:11px; color:#cc3333; padding:4px 0 0 22px;' }));
        else this._dtl(sec, pr);
        r.add_child(sec);
    }

    _dtl(sec, pr) {
        const pid=pr.provider_id, d=pr.details||{};
        if (pid==='deepseek') {
            const c=d.currency||'CNY', s=c==='USD'?'$':'\u00A5';
            for (const [l,k] of [['Balance','total_balance'],['Granted','granted_balance'],['Topped Up','topped_up_balance']]) {
                const v=+d[k]||0;
                const row=new St.BoxLayout({ style:'padding:2px 0 2px 24px;' });
                row.add_child(new St.Label({ text:l+': ', style:'font-size:11px; color:#888;' }));
                row.add_child(new St.Label({ text:`${s}${v.toFixed(2)}`, style:'font-size:11px; color:#bbb; font-family:monospace;' }));
                sec.add_child(row);
            }
        } else {
            const dn=d.plan_name||'Unknown', fh=d.five_hour_usage_left_rate!=null?Math.round(d.five_hour_usage_left_rate*100):'--', wk=d.weekly_usage_left_rate!=null?Math.round(d.weekly_usage_left_rate*100):'--';
            sec.add_child(new St.Label({ text:`Plan: ${dn}`, style:'font-size:11px; color:#3584e4; font-weight:bold; padding:2px 0 2px 24px;' }));
            sec.add_child(new St.Label({ text:`5h: ${fh}% (reset ${this._r(d.five_hour_usage_reset_time)})`, style:'font-size:11px; color:#999; padding:1px 0 1px 24px;' }));
            sec.add_child(new St.Label({ text:`Week: ${wk}% (reset ${this._r(d.weekly_usage_reset_time)})`, style:'font-size:11px; color:#999; padding:1px 0 1px 24px;' }));
            const v=Math.round(pr.remaining_percent||0), w2=190, f=Math.max(0,Math.round(v/100*w2));
            let cl='high'; if(v<20)cl='low'; else if(v<50)cl='medium';
            const bar=new St.BoxLayout({ vertical:true, style:'padding:6px 0 4px 24px;' });
            const tr=new St.BoxLayout({ style_class:'codex-bar-bar-bg', style:`width:${w2}px;` });
            tr.add_child(new St.BoxLayout({ style:`width:${f}px;`, style_class:`codex-bar-bar-fill ${cl}` }));
            bar.add_child(tr); bar.add_child(new St.Label({ text:`${v}%`, x_expand:true, style_class:`codex-bar-percent ${cl}` }));
            sec.add_child(bar);
        }
    }

    _addFt(r) {
        const f=new St.BoxLayout({ style:'padding:8px 12px; border-top:1px solid rgba(255,255,255,0.08); margin-top:4px;' });
        const st=StatusReader.readStatus();
        f.add_child(new St.Label({ text:st?.updated_at?new Date(st.updated_at).toLocaleTimeString():'Never', style:'font-size:10px; color:#666;' }));
        f.add_child(new St.Widget({ x_expand:true }));
        const btn=new St.Button({ label:'\u21BB Refresh', style_class:'codex-bar-button' });
        btn.connect('button-press-event', () => { try{GLib.spawn_command_line_async('codex-bar-cli fetch');}catch(e){} GLib.timeout_add(GLib.PRIORITY_DEFAULT,2000,()=>{this._updatePanel(); if(this._popup.visible){this._popup.destroy_all_children();this._build();} return GLib.SOURCE_REMOVE;}); return Clutter.EVENT_STOP; });
        f.add_child(btn);
        r.add_child(f);
    }

    _r(ts) { if(!ts)return'--'; try{const d=new Date(ts)-Date.now(); if(d<0)return'now'; const h=Math.floor(d/36e5),m=Math.floor((d%36e5)/6e4); if(h>24)return`${Math.floor(h/24)}d`; if(h>0)return`${h}h${m>0?m+'m':''}`; return`${m}m`;}catch(e){return'--';} }
}
