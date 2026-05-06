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

/**
 * Read and parse the status.json file.
 * @returns {Object|null} Parsed status object or null if unavailable.
 */
export function readStatus() {
    const file = Gio.File.new_for_path(STATUS_FILE_PATH);
    if (!file.query_exists(null)) {
        return null;
    }

    try {
        const [, contents] = file.load_contents(null);
        const decoder = new TextDecoder('utf-8');
        const text = decoder.decode(contents);
        return JSON.parse(text);
    } catch (e) {
        log('[codex-bar] Failed to read status.json: ' + e.message);
        return null;
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
        Gio.FileCreateFlags.REPLACE_DESTINATION,
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

    const monitor = file.monitor_file(Gio.FileMonitorFlags.NONE, null);
    monitor.connect('changed', () => {
        callback();
    });
    return monitor;
}
