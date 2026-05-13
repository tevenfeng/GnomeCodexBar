use std::time::Duration;

use chrono::Utc;

use crate::{
    config::Config,
    output,
    providers::{
        deepseek::DeepSeekProvider, opencodego::OpenCodeGoProvider, sanitize_error_message,
        stepfun::StepFunProvider, Provider, StatusSnapshot,
    },
};

pub const PROVIDER_OUTER_TIMEOUT_SECS: u64 = 65;
pub const STEPFUN_OUTER_TIMEOUT_SECS: u64 = 180;

pub async fn run_daemon() -> anyhow::Result<()> {
    log::info!("Daemon started");

    // Fetch immediately on startup so the frontends do not wait for the first
    // refresh interval before status.json is populated or refreshed.
    let mut config = load_initial_config();
    let mut interval_secs = config.general.refresh_interval_secs_clamped();
    log::info!("Initial refresh started, interval: {}s", interval_secs);
    if let Err(e) = fetch_and_write_status(&config).await {
        log::error!(
            "Initial fetch failed: {}",
            sanitize_error_message(&e.to_string())
        );
    }

    loop {
        sleep_until_next_cycle(interval_secs).await;

        // Reload config every cycle so UI changes (refresh interval / provider enabled)
        // take effect without restarting the daemon.
        match Config::load() {
            Ok(new_config) => config = new_config,
            Err(e) => log::warn!(
                "Failed to reload config; keeping last-known-good config: {}",
                sanitize_error_message(&e.to_string())
            ),
        }
        interval_secs = config.general.refresh_interval_secs_clamped();
        log::info!("Refresh cycle started, interval: {}s", interval_secs);

        if let Err(e) = fetch_and_write_status(&config).await {
            log::error!("Fetch failed: {}", sanitize_error_message(&e.to_string()));
        }
    }
}

fn load_initial_config() -> Config {
    match Config::load() {
        Ok(config) => config,
        Err(e) => {
            log::warn!(
                "Failed to load config; using defaults: {}",
                sanitize_error_message(&e.to_string())
            );
            Config::default()
        }
    }
}

pub async fn fetch_and_write_status(config: &Config) -> anyhow::Result<StatusSnapshot> {
    let snapshot = fetch_all(config).await?;
    output::write_status(&snapshot)?;
    log::info!("Status updated at {}", snapshot.updated_at);
    Ok(snapshot)
}

async fn sleep_until_next_cycle(interval_secs: u64) {
    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
}

pub async fn fetch_all(config: &Config) -> anyhow::Result<StatusSnapshot> {
    let deepseek = DeepSeekProvider::new();
    let stepfun = StepFunProvider::new();
    let opencodego = OpenCodeGoProvider::new();

    let ds_config = config.deepseek_config();
    let sf_config = config.stepfun_config();
    let og_config = config.opencodego_config();

    let ds_fetch = async {
        if ds_config.enabled {
            Some(fetch_provider(&deepseek, &ds_config).await)
        } else {
            None
        }
    };
    let sf_fetch = async {
        if sf_config.enabled {
            Some(fetch_provider(&stepfun, &sf_config).await)
        } else {
            None
        }
    };
    let og_fetch = async {
        if og_config.enabled {
            Some(fetch_provider(&opencodego, &og_config).await)
        } else {
            None
        }
    };

    let (ds_result, sf_result, og_result) = tokio::join!(ds_fetch, sf_fetch, og_fetch);

    let mut providers = Vec::new();
    providers.extend([ds_result, sf_result, og_result].into_iter().flatten());

    Ok(StatusSnapshot {
        updated_at: Utc::now().to_rfc3339(),
        providers,
    })
}

async fn fetch_provider(
    provider: &dyn Provider,
    config: &crate::providers::ProviderConfig,
) -> crate::providers::ProviderStatus {
    log::debug!("Fetching provider {} ({})", provider.id(), provider.name());
    // StepFun performs a multi-request login + usage flow and may retry once on
    // auth errors, so it needs a wider outer timeout than single-request providers.
    let timeout_secs = if provider.id() == "stepfun" {
        STEPFUN_OUTER_TIMEOUT_SECS
    } else {
        PROVIDER_OUTER_TIMEOUT_SECS
    };
    let result =
        tokio::time::timeout(Duration::from_secs(timeout_secs), provider.fetch(config)).await;

    match result {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => provider_error_status(provider, &e.to_string()),
        Err(_) => provider_error_status(
            provider,
            &format!("Provider fetch timed out after {}s", timeout_secs),
        ),
    }
}

fn provider_error_status(
    provider: &dyn Provider,
    message: &str,
) -> crate::providers::ProviderStatus {
    let sanitized = sanitize_error_message(message);
    log::error!("Provider {} fetch error: {}", provider.id(), sanitized);
    crate::providers::ProviderStatus {
        provider_id: provider.id().into(),
        provider_name: provider.name().into(),
        available: false,
        remaining_percent: 0.0,
        details: std::collections::HashMap::new(),
        error: Some(sanitized),
    }
}

#[cfg(test)]
#[path = "../tests/unit/daemon.rs"]
mod tests;
