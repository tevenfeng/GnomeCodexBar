use std::fs;

use crate::{
    config::status_dir,
    providers::StatusSnapshot,
};

/// Write status snapshot to status.json in the platform-specific data directory
/// Linux: ~/.local/share/gnome-codex-bar/status.json
/// macOS: ~/Library/Application Support/gnome-codex-bar/status.json
pub fn write_status(snapshot: &StatusSnapshot) -> anyhow::Result<()> {
    let dir = status_dir();
    fs::create_dir_all(&dir)?;

    let path = dir.join("status.json");
    let json = serde_json::to_string_pretty(snapshot)?;
    fs::write(&path, json)?;

    // Set file permissions to 0600 for privacy
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(&path, perms);
    }

    Ok(())
}

/// Write selected_provider to the status directory (used by daemon to persist selection).
pub fn write_selected_provider_sync(provider_id: &str) -> anyhow::Result<()> {
    let dir = status_dir();
    fs::create_dir_all(&dir)?;

    let path = dir.join("selected_provider.json");
    let val = serde_json::json!({ "selected_provider": provider_id });
    fs::write(&path, serde_json::to_string(&val)?)?;
    Ok(())
}

/// Read selected_provider from status directory.
#[allow(dead_code)]
pub fn read_selected_provider_sync() -> Option<String> {
    let dir = status_dir();
    let path = dir.join("selected_provider.json");
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            return val["selected_provider"].as_str().map(|s| s.to_string());
        }
    }
    None
}

#[cfg(test)]
#[path = "../tests/unit/output.rs"]
mod tests;
