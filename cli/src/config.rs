use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

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
    // StepFun token cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_ingress_cookie: Option<String>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: HashMap::from([
                ("deepseek".into(), ProviderItem {
                    enabled: true,
                    api_key: None,
                    username: None,
                    password: None,
                    cached_token: None,
                    cached_ingress_cookie: None,
                }),
                ("stepfun".into(), ProviderItem {
                    enabled: true,
                    api_key: None,
                    username: None,
                    password: None,
                    cached_token: None,
                    cached_ingress_cookie: None,
                }),
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
                let _: Result<(), std::io::Error> = fs::rename(&old_path, &path)
                    .or_else(|_| {
                        // Fall back to copy+delete if cross-device
                        let content = fs::read_to_string(&old_path)?;
                        fs::write(&path, &content)?;
                        let _ = fs::remove_file(&old_path);
                        Ok(())
                    });
            }
        }

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
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
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn deepseek_config(&self) -> super::providers::ProviderConfig {
        let p = self.providers.get("deepseek");
        super::providers::ProviderConfig {
            enabled: p.map(|p| p.enabled).unwrap_or(true),
            api_key: p.and_then(|p| p.api_key.clone()),
            username: None,
            password: None,
        }
    }

    pub fn stepfun_config(&self) -> super::providers::ProviderConfig {
        let p = self.providers.get("stepfun");
        super::providers::ProviderConfig {
            enabled: p.map(|p| p.enabled).unwrap_or(true),
            api_key: None,
            username: p.and_then(|p| p.username.clone()),
            password: p.and_then(|p| p.password.clone()),
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

#[cfg(test)]
#[path = "../tests/unit/config.rs"]
mod tests;
