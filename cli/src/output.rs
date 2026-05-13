use std::{fs, path::Path};

use crate::{atomic_write::atomic_write, config::status_dir, providers::StatusSnapshot};

/// Write status snapshot to status.json in the platform-specific data directory
/// Linux: ~/.local/share/gnome-codex-bar/status.json
/// macOS: ~/Library/Application Support/gnome-codex-bar/status.json
pub fn write_status(snapshot: &StatusSnapshot) -> anyhow::Result<()> {
    let dir = status_dir();
    fs::create_dir_all(&dir)?;

    let path = dir.join("status.json");
    let json = serde_json::to_string_pretty(snapshot)?;
    atomic_write(&path, json)?;

    Ok(())
}

/// Write selected_provider to the status directory (used by daemon to persist selection).
pub fn write_selected_provider_sync(provider_id: &str) -> anyhow::Result<()> {
    let dir = status_dir();
    fs::create_dir_all(&dir)?;

    let path = dir.join("selected_provider.json");
    write_selected_provider_to_path(&path, provider_id)?;
    Ok(())
}

fn write_selected_provider_to_path(path: &Path, provider_id: &str) -> anyhow::Result<()> {
    let val = serde_json::json!({ "selected_provider": provider_id });
    atomic_write(path, serde_json::to_string(&val)?)?;
    Ok(())
}

#[cfg(test)]
fn read_selected_provider_from_path(path: &Path) -> Option<String> {
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            return val["selected_provider"].as_str().map(|s| s.to_string());
        }
    }
    None
}

#[cfg(test)]
#[path = "../tests/unit/output.rs"]
mod tests;
