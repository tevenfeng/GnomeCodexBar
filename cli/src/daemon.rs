use std::time::Duration;

use chrono::Utc;

use crate::{
    config::Config,
    output,
    providers::{deepseek::DeepSeekProvider, stepfun::StepFunProvider, Provider, ProviderStatus, StatusSnapshot},
};

pub async fn run_daemon() -> anyhow::Result<()> {
    let config = Config::load()?;
    let interval = Duration::from_secs(config.general.refresh_interval_secs);

    log::info!(
        "Daemon started, refresh interval: {}s",
        config.general.refresh_interval_secs
    );

    loop {
        let result = fetch_all(&config).await;
        match result {
            Ok(snapshot) => {
                if let Err(e) = output::write_status(&snapshot) {
                    log::error!("Failed to write status: {}", e);
                } else {
                    log::info!("Status updated at {}", snapshot.updated_at);
                }
            }
            Err(e) => {
                log::error!("Fetch failed: {}", e);
            }
        }

        tokio::time::sleep(interval).await;
    }
}

pub async fn fetch_all(config: &Config) -> anyhow::Result<StatusSnapshot> {
    let deepseek = DeepSeekProvider::new();
    let stepfun = StepFunProvider::new();

    let ds_config = config.deepseek_config();
    let sf_config = config.stepfun_config();

    let (ds_result, sf_result) = tokio::join!(
        fetch_provider(&deepseek, &ds_config),
        fetch_provider(&stepfun, &sf_config),
    );

    let mut providers = Vec::new();
    for result in [ds_result, sf_result] {
        match result {
            Ok(status) => providers.push(status),
            Err(e) => {
                log::error!("Provider fetch error: {}", e);
            }
        }
    }

    Ok(StatusSnapshot {
        updated_at: Utc::now().to_rfc3339(),
        providers,
    })
}

async fn fetch_provider(
    provider: &dyn Provider,
    config: &crate::providers::ProviderConfig,
) -> anyhow::Result<ProviderStatus> {
    if !config.enabled {
        return Ok(ProviderStatus {
            provider_id: provider.id().into(),
            provider_name: provider.name().into(),
            available: false,
            remaining_percent: 0.0,
            details: std::collections::HashMap::new(),
            error: Some("Provider disabled".into()),
        });
    }

    provider.fetch(config).await
}
