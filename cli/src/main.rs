mod config;
mod daemon;
mod output;
mod providers;

use clap::{Parser, Subcommand};

use crate::{
    config::{Config, ProviderItem},
    daemon::fetch_all,
};

#[derive(Parser)]
#[command(name = "codex-bar-cli")]
#[command(about = "Monitor DeepSeek and StepFun coding plan usage")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure provider settings
    Config {
        /// Provider name (deepseek or stepfun)
        provider: String,
        /// Set API key (for DeepSeek)
        #[arg(long)]
        api_key: Option<String>,
        /// Set username (for StepFun)
        #[arg(long)]
        username: Option<String>,
        /// Set password (for StepFun)
        #[arg(long)]
        password: Option<String>,
        /// Enable or disable this provider
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Fetch usage data now and write to status.json
    Fetch,
    /// Run as daemon, refreshing periodically
    Daemon,
    /// Show current status from status.json
    Status,
    /// Set monthly budget for percentage calculation
    Budget {
        /// Monthly budget amount (CNY)
        amount: f64,
    },
    /// Select which provider to display in the panel
    Select {
        /// Provider id (deepseek or stepfun)
        provider: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Config {
            provider,
            api_key,
            username,
            password,
            enabled,
        } => {
            let mut config = Config::load()?;
            let entry = config
                .providers
                .entry(provider.clone())
                .or_insert_with(|| ProviderItem {
                    enabled: true,
                    api_key: None,
                    username: None,
                    password: None,
                    cached_token: None,
                    cached_ingress_cookie: None,
                });

            if let Some(key) = api_key {
                entry.api_key = Some(key);
            }
            if let Some(user) = username {
                entry.username = Some(user);
            }
            if let Some(pass) = password {
                entry.password = Some(pass);
            }
            if let Some(en) = enabled {
                entry.enabled = en;
            }

            config.save()?;
            println!("Config updated for provider '{}'", provider);
        }

        Commands::Fetch => {
            let config = Config::load()?;
            let snapshot = fetch_all(&config).await?;
            output::write_status(&snapshot)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&snapshot)?
            );
        }

        Commands::Daemon => {
            crate::daemon::run_daemon().await?;
        }

        Commands::Status => {
            let path = crate::config::status_dir().join("status.json");
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                println!("{}", content);
            } else {
                println!("No status data yet. Run 'fetch' or 'daemon' first.");
            }
        }

        Commands::Budget { amount } => {
            let mut config = Config::load()?;
            config.general.budget_monthly = Some(amount);
            config.save()?;
            println!("Monthly budget set to {}", amount);
        }

        Commands::Select { provider } => {
            output::write_selected_provider_sync(&provider)?;
            println!("Selected provider in panel: {}", provider);
        }
    }

    Ok(())
}
