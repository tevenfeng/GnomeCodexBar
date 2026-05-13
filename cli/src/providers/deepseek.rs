use std::{collections::HashMap, time::Duration};

use serde::Deserialize;

use super::{
    sanitized_http_error_message, Provider, ProviderConfig, ProviderStatus,
    HTTP_CONNECT_TIMEOUT_SECS, HTTP_REQUEST_TIMEOUT_SECS,
};

pub struct DeepSeekProvider;

impl DeepSeekProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Provider for DeepSeekProvider {
    fn id(&self) -> &'static str {
        "deepseek"
    }

    fn name(&self) -> &'static str {
        "DeepSeek"
    }

    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error> {
        let api_key = config
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("DeepSeek API key not configured"))?;

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()?;
        let resp = client
            .get("https://api.deepseek.com/user/balance")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Ok(ProviderStatus {
                provider_id: "deepseek".into(),
                provider_name: "DeepSeek".into(),
                available: false,
                remaining_percent: 0.0,
                details: HashMap::new(),
                error: Some(sanitized_http_error_message(
                    "DeepSeek balance",
                    status,
                    &body,
                )),
            });
        }

        let body: DeepSeekBalanceResponse = resp.json().await?;

        let mut details = HashMap::new();
        let remaining_percent: f64;

        if let Some(info) = body.balance_infos.first() {
            let total_balance: f64 = info.total_balance.parse().unwrap_or(0.0);
            let granted: f64 = info.granted_balance.parse().unwrap_or(0.0);
            let topped_up: f64 = info.topped_up_balance.parse().unwrap_or(0.0);

            details.insert(
                "currency".into(),
                serde_json::Value::String(info.currency.clone()),
            );
            details.insert(
                "total_balance".into(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(total_balance)
                        .unwrap_or(serde_json::Number::from(0)),
                ),
            );
            details.insert(
                "granted_balance".into(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(granted).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            details.insert(
                "topped_up_balance".into(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(topped_up).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            details.insert(
                "is_available".into(),
                serde_json::Value::Bool(body.is_available),
            );

            if total_balance > 0.0 {
                remaining_percent = 100.0;
            } else {
                remaining_percent = 0.0;
            }
        } else {
            remaining_percent = 0.0;
        }

        Ok(ProviderStatus {
            provider_id: "deepseek".into(),
            provider_name: "DeepSeek".into(),
            available: body.is_available,
            remaining_percent,
            details,
            error: None,
        })
    }
}

#[derive(Debug, Deserialize)]
struct DeepSeekBalanceResponse {
    is_available: bool,
    balance_infos: Vec<DeepSeekBalanceInfo>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekBalanceInfo {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

#[cfg(test)]
#[path = "../../tests/unit/providers_deepseek.rs"]
mod tests;
