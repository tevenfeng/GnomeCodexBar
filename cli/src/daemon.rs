use std::time::Duration;

use chrono::Utc;

use crate::{
    config::Config,
    output,
    providers::{
        deepseek::DeepSeekProvider, opencodego::OpenCodeGoProvider, stepfun::StepFunProvider,
        Provider, StatusSnapshot,
    },
};

pub async fn run_daemon() -> anyhow::Result<()> {
    log::info!("Daemon started");

    // Fetch immediately on startup so the frontends do not wait for the first
    // refresh interval before status.json is populated or refreshed.
    let mut config = Config::load()?;
    let mut interval_secs = config.general.refresh_interval_secs;
    log::info!("Initial refresh started, interval: {}s", interval_secs);
    if let Err(e) = fetch_and_write_status(&config).await {
        log::error!("Initial fetch failed: {}", e);
    }

    loop {
        sleep_until_next_cycle(interval_secs).await;

        // Reload config every cycle so UI changes (refresh interval / provider enabled)
        // take effect without restarting the daemon.
        config = Config::load()?;
        interval_secs = config.general.refresh_interval_secs;
        log::info!("Refresh cycle started, interval: {}s", interval_secs);

        if let Err(e) = fetch_and_write_status(&config).await {
            log::error!("Fetch failed: {}", e);
        }
    }
}

pub async fn fetch_and_write_status(config: &Config) -> anyhow::Result<StatusSnapshot> {
    let snapshot = fetch_all(config).await?;
    output::write_status(&snapshot)?;
    log::info!("Status updated at {}", snapshot.updated_at);
    Ok(snapshot)
}

async fn sleep_until_next_cycle(initial_interval_secs: u64) {
    let mut elapsed_secs = 0;

    loop {
        let current_interval_secs = Config::load()
            .map(|config| config.general.refresh_interval_secs)
            .unwrap_or(initial_interval_secs);

        if elapsed_secs >= current_interval_secs {
            break;
        }

        let step_secs = (current_interval_secs - elapsed_secs).min(1);
        tokio::time::sleep(Duration::from_secs(step_secs)).await;
        elapsed_secs += step_secs;
    }
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
    for result in [ds_result, sf_result, og_result] {
        match result.transpose() {
            Ok(Some(status)) => providers.push(status),
            Ok(None) => {}
            Err(e) if e.to_string().starts_with("Provider disabled:") => {
                log::debug!("Provider skipped: {}", e);
            }
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
) -> anyhow::Result<crate::providers::ProviderStatus> {
    provider.fetch(config).await
}
