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
    /// Load config from ~/.config/codex-bar/config.toml.
    /// Creates with defaults if file doesn't exist.
    pub fn load() -> anyhow::Result<Self> {
        let path = config_path();
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
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("codex-bar")
        .join("config.toml")
}

pub fn status_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gnome-codex-bar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        // Both providers should exist
        assert!(config.providers.contains_key("deepseek"));
        assert!(config.providers.contains_key("stepfun"));

        // Both should be enabled
        assert!(config.providers["deepseek"].enabled);
        assert!(config.providers["stepfun"].enabled);

        // General config defaults
        assert_eq!(config.general.refresh_interval_secs, 300);
        assert_eq!(config.general.selected_provider, "deepseek");
        assert!(config.general.budget_monthly.is_none());
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let mut config = Config::default();
        config.providers.get_mut("deepseek").unwrap().api_key = Some("sk-test123".into());
        config.providers.get_mut("stepfun").unwrap().username = Some("user@example.com".into());
        config.providers.get_mut("stepfun").unwrap().password = Some("secret".into());
        config.general.refresh_interval_secs = 60;
        config.general.budget_monthly = Some(100.0);
        config.general.selected_provider = "stepfun".into();

        let toml_str = toml::to_string_pretty(&config).expect("serialize to TOML");
        let config2: Config = toml::from_str(&toml_str).expect("deserialize from TOML");

        assert_eq!(config2.providers["deepseek"].api_key, Some("sk-test123".into()));
        assert_eq!(config2.providers["deepseek"].enabled, true);
        assert_eq!(config2.providers["stepfun"].username, Some("user@example.com".into()));
        assert_eq!(config2.providers["stepfun"].password, Some("secret".into()));
        assert_eq!(config2.general.refresh_interval_secs, 60);
        assert_eq!(config2.general.budget_monthly, Some(100.0));
        assert_eq!(config2.general.selected_provider, "stepfun");
    }

    #[test]
    fn test_provider_item_skip_serializing() {
        let item = ProviderItem {
            enabled: true,
            api_key: None,
            username: None,
            password: None,
            cached_token: None,
            cached_ingress_cookie: None,
        };

        let toml_str = toml::to_string_pretty(&item).expect("serialize");

        // None fields should be absent from the TOML output
        assert!(!toml_str.contains("api_key"));
        assert!(!toml_str.contains("username"));
        assert!(!toml_str.contains("password"));
        assert!(!toml_str.contains("cached_token"));
        assert!(!toml_str.contains("cached_ingress_cookie"));

        // enabled should still be present
        assert!(toml_str.contains("enabled"));
    }

    #[test]
    fn test_deepseek_config() {
        let mut config = Config::default();
        config.providers.get_mut("deepseek").unwrap().api_key = Some("sk-abc".into());

        let ds = config.deepseek_config();
        assert!(ds.enabled);
        assert_eq!(ds.api_key, Some("sk-abc".into()));
        assert!(ds.username.is_none());
        assert!(ds.password.is_none());
    }

    #[test]
    fn test_stepfun_config() {
        let mut config = Config::default();
        config.providers.get_mut("stepfun").unwrap().username = Some("user@test.com".into());
        config.providers.get_mut("stepfun").unwrap().password = Some("pass".into());

        let sf = config.stepfun_config();
        assert!(sf.enabled);
        assert!(sf.api_key.is_none());
        assert_eq!(sf.username, Some("user@test.com".into()));
        assert_eq!(sf.password, Some("pass".into()));
    }

    #[test]
    fn test_config_missing_provider() {
        let config = Config {
            providers: HashMap::new(),
            general: GeneralConfig::default(),
        };

        // When provider is missing, enabled should default to true
        let ds = config.deepseek_config();
        assert!(ds.enabled);
        assert!(ds.api_key.is_none());

        let sf = config.stepfun_config();
        assert!(sf.enabled);
        assert!(sf.username.is_none());
        assert!(sf.password.is_none());
    }
}
