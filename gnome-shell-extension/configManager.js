// configManager.js — lightweight line-level TOML reader/writer
// Reads/writes only [general].refresh_interval_secs and [providers.X].enabled
// All other TOML content is preserved verbatim.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

const CFG_DIR = GLib.build_filenamev([GLib.get_user_data_dir(), 'gnome-codex-bar']);
const CFG_PATH = GLib.build_filenamev([CFG_DIR, 'config.toml']);

function _readLines() {
    const file = Gio.File.new_for_path(CFG_PATH);
    if (!file.query_exists(null)) return null;
    try {
        const [ok, contents] = file.load_contents(null);
        if (!ok) return null;
        return new TextDecoder().decode(contents).split('\n');
    } catch (e) {
        return null;
    }
}

function _writeLines(lines) {
    // Ensure directory exists
    const dir = Gio.File.new_for_path(CFG_DIR);
    if (!dir.query_exists(null)) dir.make_directory_with_parents(null);
    // Write to temp file, then rename
    const content = lines.join('\n');
    const tmpPath = CFG_PATH + '.tmp';
    try { GLib.file_set_contents(tmpPath, content); } catch (e) { return false; }
    return GLib.file_set_contents(CFG_PATH, content);
}

// Find the index of [section] in lines, starting from startIdx
function _findSection(lines, section, startIdx = 0) {
    const target = '[' + section + ']';
    for (let i = startIdx; i < lines.length; i++) {
        if (lines[i].trim() === target) return i;
    }
    return -1;
}

// Find key=value line index within a section
function _findKey(lines, key, sectionStart, sectionEnd) {
    const prefix = key + ' = ';
    for (let i = sectionStart + 1; i < sectionEnd; i++) {
        if (lines[i].trimStart().startsWith(prefix)) return i;
    }
    return -1;
}

// Get the end index of a section (start of next section or EOF)
function _sectionEnd(lines, sectionStart) {
    for (let i = sectionStart + 1; i < lines.length; i++) {
        if (lines[i].trimStart().startsWith('[') && !lines[i].trimStart().startsWith('[')) continue;
        if (/^\[.*\]\s*$/.test(lines[i].trim())) return i;
    }
    return lines.length;
}

// Read a string value from config
function _readString(lines, section, key) {
    const si = _findSection(lines, section);
    if (si < 0) return null;
    const ei = _sectionEnd(lines, si);
    const ki = _findKey(lines, key, si, ei);
    if (ki < 0) return null;
    const line = lines[ki].trim();
    const eqIdx = line.indexOf('=');
    if (eqIdx < 0) return null;
    let val = line.substring(eqIdx + 1).trim();
    if (val.startsWith('"') && val.endsWith('"')) val = val.slice(1, -1);
    return val;
}

// Read an int value from config
function _readInt(lines, section, key) {
    const s = _readString(lines, section, key);
    if (s === null) return null;
    const n = parseInt(s, 10);
    return isNaN(n) ? null : n;
}

// Read a bool value from config
function _readBool(lines, section, key) {
    const s = _readString(lines, section, key);
    if (s === null) return null;
    return s === 'true';
}

// Write a key=value (int or bool) into a section, preserving all other content
function _writeKey(lines, section, key, value, quoted = false) {
    const valStr = quoted ? `"${String(value)}"` : String(value);
    const newLine = key + ' = ' + valStr;
    let si = _findSection(lines, section);
    if (si < 0) {
        // Section doesn't exist — append
        lines.push('');
        lines.push('[' + section + ']');
        lines.push(newLine);
        return;
    }
    const ei = _sectionEnd(lines, si);
    const ki = _findKey(lines, key, si, ei);
    if (ki >= 0) {
        // Replace existing line
        const indent = lines[ki].length - lines[ki].trimStart().length;
        lines[ki] = ' '.repeat(indent) + newLine;
    } else {
        // Insert after section header (preserve blank line after header)
        const insertAt = (lines[si + 1] && lines[si + 1].trim() === '') ? si + 2 : si + 1;
        lines.splice(insertAt, 0, newLine);
    }
}

// ─── Public API ───

export class ConfigManager {
    constructor() {
        this._lines = _readLines();
        this._needsWrite = false;
    }

    // Reload from disk (call after file monitor triggers)
    reload() {
        this._lines = _readLines();
        return this;
    }

    get path() {
        return CFG_PATH;
    }

    getRefreshInterval() {
        if (!this._lines) return null;
        return _readInt(this._lines, 'general', 'refresh_interval_secs');
    }

    setRefreshInterval(seconds) {
        if (!this._lines) this._lines = [];
        _writeKey(this._lines, 'general', 'refresh_interval_secs', seconds);
        _writeLines(this._lines);
    }

    isProviderEnabled(providerId) {
        if (!this._lines) return true; // default: enabled
        const v = _readBool(this._lines, 'providers.' + providerId, 'enabled');
        return v === null ? true : v;
    }

    setProviderEnabled(providerId, enabled) {
        if (!this._lines) this._lines = [];
        _writeKey(this._lines, 'providers.' + providerId, 'enabled', enabled);
        _writeLines(this._lines);
    }

    // Get all provider enabled states
    getProviderEnabledMap() {
        const map = {};
        for (const pid of ['deepseek', 'stepfun', 'opencodego']) {
            map[pid] = this.isProviderEnabled(pid);
        }
        return map;
    }

    // Monitor config.toml changes via Gio.FileMonitor
    monitor(onChanged) {
        const file = Gio.File.new_for_path(CFG_PATH);
        // Ensure file exists (create minimal config)
        if (!file.query_exists(null)) {
            const dir = Gio.File.new_for_path(CFG_DIR);
            if (!dir.query_exists(null)) dir.make_directory_with_parents(null);
            const defaults = [
                '[providers.deepseek]',
                'enabled = true',
                '',
                '[providers.stepfun]',
                'enabled = true',
                '',
                '[providers.opencodego]',
                'enabled = false',
                '',
                '[general]',
                'refresh_interval_secs = 300',
                'selected_provider = "deepseek"',
                '',
            ];
            try { GLib.file_set_contents(CFG_PATH, defaults.join('\n')); } catch (e) {}
        }
        try {
            this._monitor = file.monitor_file(Gio.FileMonitorFlags.NONE, null);
            this._monitor.connect('changed', () => {
                this.reload();
                if (onChanged) onChanged();
            });
        } catch (e) {
            log(`[codex-bar] Failed to monitor config.toml: ${e}`);
        }
    }

    destroy() {
        if (this._monitor) {
            this._monitor.cancel();
            this._monitor = null;
        }
    }
}
