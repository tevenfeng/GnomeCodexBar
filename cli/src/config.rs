use std::{collections::HashMap, fs, path::PathBuf};

use crate::atomic_write::atomic_write;

use serde::{Deserialize, Serialize};

pub const MIN_REFRESH_INTERVAL_SECS: u64 = 30;
pub const MAX_REFRESH_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: HashMap<String, ProviderItem>,
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderItem {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_monthly: Option<f64>,
    #[serde(default = "default_selected_provider")]
    pub selected_provider: String,
}

fn default_true() -> bool {
    true
}
fn default_refresh_interval() -> u64 {
    300
}
fn default_selected_provider() -> String {
    "deepseek".into()
}

impl GeneralConfig {
    pub fn refresh_interval_secs_clamped(&self) -> u64 {
        self.refresh_interval_secs
            .clamp(MIN_REFRESH_INTERVAL_SECS, MAX_REFRESH_INTERVAL_SECS)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: HashMap::from([
                (
                    "deepseek".into(),
                    ProviderItem {
                        enabled: true,
                        api_key: None,
                        username: None,
                        password: None,
                        cookie_header: None,
                        workspace_id: None,
                    },
                ),
                (
                    "stepfun".into(),
                    ProviderItem {
                        enabled: true,
                        api_key: None,
                        username: None,
                        password: None,
                        cookie_header: None,
                        workspace_id: None,
                    },
                ),
                (
                    "opencodego".into(),
                    ProviderItem {
                        enabled: false,
                        api_key: None,
                        username: None,
                        password: None,
                        cookie_header: None,
                        workspace_id: None,
                    },
                ),
            ]),
            general: GeneralConfig {
                refresh_interval_secs: 300,
                budget_monthly: None,
                selected_provider: "deepseek".into(),
            },
        }
    }
}

impl Config {
    fn clamp_refresh_interval(mut self) -> Self {
        self.general.refresh_interval_secs = self.general.refresh_interval_secs_clamped();
        self
    }

    /// Load config from the data directory (same dir as status.json).
    /// Creates with defaults if file doesn't exist.
    /// Migrates from the old config directory if the old file exists but the new one doesn't.
    pub fn load() -> anyhow::Result<Self> {
        let path = config_path();

        // Migrate from old config location if needed
        if !path.exists() {
            let old_path = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("codex-bar")
                .join("config.toml");
            if old_path.exists() {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let migration_result: Result<(), std::io::Error> = (|| {
                    let content = fs::read_to_string(&old_path)?;
                    atomic_write(&path, content)?;
                    if let Err(e) = fs::remove_file(&old_path) {
                        restrict_file_permissions(&old_path);
                        log::warn!(
                            "Migrated config to {}, but failed to remove old config {}: {}",
                            path.display(),
                            old_path.display(),
                            e
                        );
                    }
                    Ok(())
                })();
                if let Err(e) = migration_result {
                    log::warn!(
                        "Failed to migrate config from {} to {}: {}",
                        old_path.display(),
                        path.display(),
                        e
                    );
                }
            }
        }

        if path.exists() {
            restrict_file_permissions(&path);
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config.clamp_refresh_interval())
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save config to file.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.clone().clamp_refresh_interval())?;
        atomic_write(&path, content)?;
        Ok(())
    }

    pub fn deepseek_config(&self) -> super::providers::ProviderConfig {
        let p = self.providers.get("deepseek");
        super::providers::ProviderConfig {
            enabled: p.map(|p| p.enabled).unwrap_or(true),
            api_key: p.and_then(|p| p.api_key.clone()),
            username: None,
            password: None,
            cookie_header: None,
            workspace_id: None,
        }
    }

    pub fn stepfun_config(&self) -> super::providers::ProviderConfig {
        let p = self.providers.get("stepfun");
        super::providers::ProviderConfig {
            enabled: p.map(|p| p.enabled).unwrap_or(true),
            api_key: None,
            username: p.and_then(|p| p.username.clone()),
            password: p.and_then(|p| p.password.clone()),
            cookie_header: None,
            workspace_id: None,
        }
    }

    pub fn opencodego_config(&self) -> super::providers::ProviderConfig {
        let p = self.providers.get("opencodego");
        super::providers::ProviderConfig {
            enabled: p.map(|p| p.enabled).unwrap_or(false),
            api_key: None,
            username: None,
            password: None,
            cookie_header: p.and_then(|p| p.cookie_header.clone()),
            workspace_id: p.and_then(|p| p.workspace_id.clone()),
        }
    }
}

fn config_path() -> PathBuf {
    status_dir().join("config.toml")
}

pub fn status_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gnome-codex-bar")
}

#[cfg(unix)]
fn restrict_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        log::warn!(
            "Failed to set private permissions on {}: {}",
            path.display(),
            e
        );
    }
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &std::path::Path) {}

#[cfg(test)]
#[path = "../tests/unit/config.rs"]
mod tests;
