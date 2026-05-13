import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const STATUS_FILE_PATH = GLib.build_filenamev([
    GLib.get_user_data_dir(),
    'gnome-codex-bar',
    'status.json',
]);

const SELECTED_PROVIDER_FILE_PATH = GLib.build_filenamev([
    GLib.get_user_data_dir(),
    'gnome-codex-bar',
    'selected_provider.json',
]);

let _lastGoodStatus = null;

/**
 * Read and parse the status.json file.
 * @returns {Object|null} Parsed status object or null if unavailable.
 */
export function readStatus() {
    const file = Gio.File.new_for_path(STATUS_FILE_PATH);
    if (!file.query_exists(null)) {
        return _lastGoodStatus;
    }

    try {
        const [, contents] = file.load_contents(null);
        const decoder = new TextDecoder('utf-8');
        const text = decoder.decode(contents);
        _lastGoodStatus = JSON.parse(text);
        return _lastGoodStatus;
    } catch (e) {
        log('[codex-bar] Failed to read status.json: ' + e.message);
        return _lastGoodStatus;
    }
}

/**
 * Read the selected provider preference.
 * @returns {string} Provider id or 'deepseek' as default.
 */
export function readSelectedProvider() {
    const file = Gio.File.new_for_path(SELECTED_PROVIDER_FILE_PATH);
    if (!file.query_exists(null)) {
        return 'deepseek';
    }

    try {
        const [, contents] = file.load_contents(null);
        const decoder = new TextDecoder('utf-8');
        const text = decoder.decode(contents);
        const data = JSON.parse(text);
        return data.selected_provider || 'deepseek';
    } catch (e) {
        return 'deepseek';
    }
}

/**
 * Write the selected provider preference.
 * @param {string} providerId
 */
export function writeSelectedProvider(providerId) {
    const file = Gio.File.new_for_path(SELECTED_PROVIDER_FILE_PATH);
    const parent = file.get_parent();
    if (!parent.query_exists(null)) {
        parent.make_directory_with_parents(null);
    }

    const encoder = new TextEncoder();
    const data = JSON.stringify({ selected_provider: providerId });
    file.replace_contents(
        encoder.encode(data),
        null,
        false,
        Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE,
        null,
    );
}

/**
 * Set up a file monitor on the status.json for live updates.
 * @param {Function} callback - Called when status file changes.
 * @returns {Gio.FileMonitor}
 */
export function monitorStatus(callback) {
    const file = Gio.File.new_for_path(STATUS_FILE_PATH);
    // Ensure parent directory exists
    const parent = file.get_parent();
    if (!parent.query_exists(null)) {
        parent.make_directory_with_parents(null);
    }

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

    const monitor = parent.monitor_directory(Gio.FileMonitorFlags.NONE, null);
    monitor.connect('changed', (_monitor, changedFile, otherFile, eventType) => {
        if (!interestingEvents.has(eventType))
            return;
        const changedName = changedFile ? changedFile.get_basename() : null;
        const otherName = otherFile ? otherFile.get_basename() : null;
        if (changedName === basename || otherName === basename)
            callback();
    });
    return monitor;
}
