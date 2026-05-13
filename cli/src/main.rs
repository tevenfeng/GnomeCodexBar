mod atomic_write;
mod autostart;
mod config;
mod daemon;
mod output;
mod providers;

use clap::{Parser, Subcommand};

use crate::{
    config::{Config, ProviderItem},
    daemon::fetch_and_write_status,
};

#[derive(Parser)]
#[command(name = "codex-bar-cli")]
#[command(about = "Monitor DeepSeek, StepFun, and OpenCode Go coding plan usage")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure provider settings
    Config {
        /// Provider name (deepseek, stepfun, or opencodego)
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
        /// Set Cookie header (for OpenCode Go)
        #[arg(long)]
        cookie_header: Option<String>,
        /// Set workspace ID (for OpenCode Go, e.g. wrk_...)
        #[arg(long)]
        workspace_id: Option<String>,
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
        /// Provider id (deepseek, stepfun, or opencodego)
        provider: String,
    },
    /// Manage auto-start of the daemon on login
    Autostart {
        /// Action: enable, disable, or status
        action: String,
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
            cookie_header,
            workspace_id,
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
                    cookie_header: None,
                    workspace_id: None,
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
            if let Some(cookie) = cookie_header {
                entry.cookie_header = Some(cookie);
            }
            if let Some(workspace) = workspace_id {
                entry.workspace_id = Some(workspace);
            }
            if let Some(en) = enabled {
                entry.enabled = en;
            }

            config.save()?;
            println!("Config updated for provider '{}'", provider);
        }

        Commands::Fetch => {
            let config = Config::load()?;
            let snapshot = fetch_and_write_status(&config).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
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

        Commands::Autostart { action } => match action.as_str() {
            "enable" => {
                let msg = autostart::enable()?;
                println!("{}", msg);
            }
            "disable" => {
                let msg = autostart::disable()?;
                println!("{}", msg);
            }
            "status" => {
                let msg = autostart::status()?;
                println!("{}", msg);
            }
            _ => {
                anyhow::bail!(
                    "Unknown autostart action '{}'. Use: enable, disable, or status",
                    action
                );
            }
        },
    }

    Ok(())
}
