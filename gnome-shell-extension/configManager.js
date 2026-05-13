// configManager.js — lightweight line-level TOML reader/writer
// Reads/writes only [general].refresh_interval_secs/provider_order and [providers.X].enabled
// All other TOML content is preserved verbatim.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

const CFG_DIR = GLib.build_filenamev([GLib.get_user_data_dir(), 'gnome-codex-bar']);
const CFG_PATH = GLib.build_filenamev([CFG_DIR, 'config.toml']);
const DEFAULT_PROVIDER_ORDER = ['deepseek', 'stepfun', 'opencodego'];

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
    const content = lines.join('\n');
    const file = Gio.File.new_for_path(CFG_PATH);
    try {
        file.replace_contents(
            new TextEncoder().encode(content),
            null,
            false,
            Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE,
            null,
        );
        return true;
    } catch (e) {
        log(`[codex-bar] Failed to write config.toml: ${e}`);
        return false;
    }
}

function _stripInlineComment(value) {
    let quote = null;
    let escaped = false;
    for (let i = 0; i < value.length; i++) {
        const ch = value[i];
        if (escaped) {
            escaped = false;
            continue;
        }
        if (ch === '\\' && quote === '"') {
            escaped = true;
            continue;
        }
        if ((ch === '"' || ch === "'") && quote === null) {
            quote = ch;
            continue;
        }
        if (ch === quote) {
            quote = null;
            continue;
        }
        if (ch === '#' && quote === null)
            return value.substring(0, i).trimEnd();
    }
    return value.trimEnd();
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
    let val = _stripInlineComment(line.substring(eqIdx + 1)).trim();
    if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'")))
        val = val.slice(1, -1);
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
    if (s === 'true') return true;
    if (s === 'false') return false;
    return null;
}

function _decodeTomlString(value) {
    value = value.trim();
    if (value.startsWith('"') && value.endsWith('"')) {
        try {
            return JSON.parse(value);
        } catch (e) {
            return value.slice(1, -1);
        }
    }
    if (value.startsWith("'") && value.endsWith("'"))
        return value.slice(1, -1);
    return null;
}

function _readStringArray(lines, section, key) {
    const si = _findSection(lines, section);
    if (si < 0) return null;
    const ei = _sectionEnd(lines, si);
    const ki = _findKey(lines, key, si, ei);
    if (ki < 0) return null;
    const line = lines[ki].trim();
    const eqIdx = line.indexOf('=');
    if (eqIdx < 0) return null;
    const val = _stripInlineComment(line.substring(eqIdx + 1)).trim();
    if (!val.startsWith('[') || !val.endsWith(']')) return null;

    const result = [];
    const inner = val.slice(1, -1);
    let quote = null;
    let escaped = false;
    let part = '';
    for (const ch of inner) {
        if (escaped) {
            part += ch;
            escaped = false;
            continue;
        }
        if (ch === '\\' && quote === '"') {
            part += ch;
            escaped = true;
            continue;
        }
        if ((ch === '"' || ch === "'") && quote === null) {
            quote = ch;
            part += ch;
            continue;
        }
        if (ch === quote) {
            quote = null;
            part += ch;
            continue;
        }
        if (ch === ',' && quote === null) {
            const decoded = _decodeTomlString(part);
            if (decoded !== null) result.push(decoded);
            part = '';
            continue;
        }
        part += ch;
    }
    const decoded = _decodeTomlString(part);
    if (decoded !== null) result.push(decoded);
    return result;
}

function _tomlString(value) {
    return JSON.stringify(String(value));
}

function _tomlStringArray(values) {
    return `[${values.map(_tomlString).join(', ')}]`;
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
        return _writeLines(this._lines);
    }

    getProviderOrder() {
        if (!this._lines) return [...DEFAULT_PROVIDER_ORDER];
        return _readStringArray(this._lines, 'general', 'provider_order') || [...DEFAULT_PROVIDER_ORDER];
    }

    setProviderOrder(order) {
        if (!this._lines) this._lines = [];
        _writeKey(this._lines, 'general', 'provider_order', _tomlStringArray(order));
        return _writeLines(this._lines);
    }

    isProviderEnabled(providerId) {
        if (!this._lines) return true; // default: enabled
        const v = _readBool(this._lines, 'providers.' + providerId, 'enabled');
        return v === null ? true : v;
    }

    setProviderEnabled(providerId, enabled) {
        if (!this._lines) this._lines = [];
        _writeKey(this._lines, 'providers.' + providerId, 'enabled', enabled);
        return _writeLines(this._lines);
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
                'provider_order = ["deepseek", "stepfun", "opencodego"]',
                '',
            ];
            _writeLines(defaults);
        }
        try {
            const dir = Gio.File.new_for_path(CFG_DIR);
            const basename = file.get_basename();
            const interestingEvents = new Set([
                Gio.FileMonitorEvent.CREATED,
                Gio.FileMonitorEvent.CHANGED,
                Gio.FileMonitorEvent.CHANGES_DONE_HINT,
                Gio.FileMonitorEvent.DELETED,
                Gio.FileMonitorEvent.MOVED,
                Gio.FileMonitorEvent.MOVED_IN,
                Gio.FileMonitorEvent.MOVED_OUT,
                Gio.FileMonitorEvent.RENAMED,
            ]);
            this._monitor = dir.monitor_directory(Gio.FileMonitorFlags.NONE, null);
            this._monitor.connect('changed', (_monitor, changedFile, otherFile, eventType) => {
                if (!interestingEvents.has(eventType))
                    return;
                const changedName = changedFile ? changedFile.get_basename() : null;
                const otherName = otherFile ? otherFile.get_basename() : null;
                if (changedName !== basename && otherName !== basename)
                    return;
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
