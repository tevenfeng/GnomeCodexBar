use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Shared trait for all provider usage fetchers.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error>;
}

/// Per-provider configuration (from config.toml).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub enabled: bool,
    // DeepSeek
    pub api_key: Option<String>,
    // StepFun
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Unified status for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider_id: String,
    pub provider_name: String,
    pub available: bool,
    /// Main percentage to display (0-100).
    pub remaining_percent: f64,
    /// Provider-specific detail fields.
    pub details: HashMap<String, serde_json::Value>,
    pub error: Option<String>,
}

/// The full status snapshot written to status.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub updated_at: String,
    pub providers: Vec<ProviderStatus>,
}

pub mod deepseek;
pub mod stepfun;
