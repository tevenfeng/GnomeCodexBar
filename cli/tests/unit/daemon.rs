use std::collections::HashMap;

use crate::providers::{ProviderConfig, ProviderStatus};

use super::*;

struct FailingProvider;

#[async_trait::async_trait]
impl Provider for FailingProvider {
    fn id(&self) -> &'static str {
        "failing"
    }

    fn name(&self) -> &'static str {
        "Failing Provider"
    }

    async fn fetch(&self, _config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error> {
        Err(anyhow::anyhow!(
            "request failed with Authorization: Bearer secret-token"
        ))
    }
}

#[tokio::test]
async fn test_fetch_provider_returns_error_status_on_failure() {
    let provider = FailingProvider;
    let status = fetch_provider(&provider, &ProviderConfig::default()).await;

    assert_eq!(status.provider_id, "failing");
    assert_eq!(status.provider_name, "Failing Provider");
    assert!(!status.available);
    assert_eq!(status.remaining_percent, 0.0);
    assert!(status.details.is_empty());
    assert_eq!(
        status.error,
        Some("request failed with Authorization: <redacted>".into())
    );
}

#[tokio::test]
async fn test_fetch_all_omits_disabled_providers() {
    let mut config = Config::default();
    for provider in config.providers.values_mut() {
        provider.enabled = false;
    }

    let snapshot = fetch_all(&config).await.expect("fetch all");

    assert!(snapshot.providers.is_empty());
}

#[test]
fn test_provider_error_status_uses_empty_details_and_sanitized_error() {
    let provider = FailingProvider;
    let status = provider_error_status(
        &provider,
        "cookie=secret; url=https://example.test/path?token=secret",
    );

    assert_eq!(status.provider_id, "failing");
    assert_eq!(status.provider_name, "Failing Provider");
    assert!(!status.available);
    assert_eq!(status.remaining_percent, 0.0);
    assert_eq!(status.details, HashMap::new());
    assert_eq!(
        status.error,
        Some("cookie=<redacted>; url=https://example.test/path?<redacted>".into())
    );
}
