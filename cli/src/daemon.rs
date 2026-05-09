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

    loop {
        // Reload config every cycle so UI changes (refresh interval / provider enabled)
        // take effect without restarting the daemon.
        let config = Config::load()?;
        let interval_secs = config.general.refresh_interval_secs;
        log::info!("Refresh cycle started, interval: {}s", interval_secs);

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

        sleep_until_next_cycle(interval_secs).await;
    }
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
