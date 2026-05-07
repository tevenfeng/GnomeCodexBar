use std::collections::HashMap;

use serde::Deserialize;

use super::{Provider, ProviderConfig, ProviderStatus};

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

        let client = reqwest::Client::new();
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
                error: Some(format!("HTTP {}: {}", status, body)),
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
                serde_json::Value::Number(serde_json::Number::from_f64(total_balance).unwrap_or(serde_json::Number::from(0))),
            );
            details.insert(
                "granted_balance".into(),
                serde_json::Value::Number(serde_json::Number::from_f64(granted).unwrap_or(serde_json::Number::from(0))),
            );
            details.insert(
                "topped_up_balance".into(),
                serde_json::Value::Number(serde_json::Number::from_f64(topped_up).unwrap_or(serde_json::Number::from(0))),
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
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_balance_response_deserialize() {
        let json = r#"{
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "10.5",
                    "granted_balance": "5.0",
                    "topped_up_balance": "5.5"
                }
            ]
        }"#;

        let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
        assert!(resp.is_available);
        assert_eq!(resp.balance_infos.len(), 1);
        let info = &resp.balance_infos[0];
        assert_eq!(info.currency, "CNY");
        assert_eq!(info.total_balance, "10.5");
        assert_eq!(info.granted_balance, "5.0");
        assert_eq!(info.topped_up_balance, "5.5");
    }

    #[test]
    fn test_deepseek_balance_info_string_parsing() {
        let json = r#"{
            "currency": "USD",
            "total_balance": "100.25",
            "granted_balance": "50.75",
            "topped_up_balance": "49.50"
        }"#;

        let info: DeepSeekBalanceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.currency, "USD");
        assert_eq!(info.total_balance, "100.25");
        assert_eq!(info.granted_balance, "50.75");
        assert_eq!(info.topped_up_balance, "49.50");

        // Verify the string fields parse correctly to f64
        let total: f64 = info.total_balance.parse().unwrap();
        let granted: f64 = info.granted_balance.parse().unwrap();
        let topped_up: f64 = info.topped_up_balance.parse().unwrap();
        assert!((total - 100.25).abs() < f64::EPSILON);
        assert!((granted - 50.75).abs() < f64::EPSILON);
        assert!((topped_up - 49.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deepseek_empty_balance_infos() {
        let json = r#"{
            "is_available": true,
            "balance_infos": []
        }"#;

        let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
        assert!(resp.is_available);
        assert!(resp.balance_infos.is_empty());

        // Empty balance_infos should yield remaining_percent = 0%
        // (matches the logic in fetch: else branch when .first() returns None)
        let remaining_percent: f64 = if resp.balance_infos.first().is_some() { 100.0 } else { 0.0 };
        assert!((remaining_percent - 0.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deepseek_provider_id_and_name() {
        let provider = DeepSeekProvider::new();
        assert_eq!(provider.id(), "deepseek");
        assert_eq!(provider.name(), "DeepSeek");
    }

    #[test]
    fn test_deepseek_remaining_percent_positive_balance() {
        let json = r#"{
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "10.5",
                    "granted_balance": "5.0",
                    "topped_up_balance": "5.5"
                }
            ]
        }"#;

        let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
        let info = resp.balance_infos.first().unwrap();
        let total_balance: f64 = info.total_balance.parse().unwrap_or(0.0);
        let remaining_percent: f64 = if total_balance > 0.0 { 100.0 } else { 0.0 };
        assert!((remaining_percent - 100.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deepseek_remaining_percent_zero_balance() {
        let json = r#"{
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "0",
                    "granted_balance": "0",
                    "topped_up_balance": "0"
                }
            ]
        }"#;

        let resp: DeepSeekBalanceResponse = serde_json::from_str(json).unwrap();
        let info = resp.balance_infos.first().unwrap();
        let total_balance: f64 = info.total_balance.parse().unwrap_or(0.0);
        let remaining_percent: f64 = if total_balance > 0.0 { 100.0 } else { 0.0 };
        assert!((remaining_percent - 0.0_f64).abs() < f64::EPSILON);
    }
}
