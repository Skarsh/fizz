pub mod agent;
pub mod config;
mod logging;
pub mod model;
pub mod model_gateway;
pub mod providers;
pub mod repl;

use anyhow::{Context, Result};
use reqwest::Client;
use std::env;
use std::time::Duration;
use tracing::info;

use agent::Agent;
use config::Config;
use repl::run_repl;

pub async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    logging::init();

    let cfg = Config::from_env();
    info!(
        model_provider = %cfg.model_provider,
        model = %cfg.model,
        model_base_url = %cfg.model_base_url,
        model_timeout_secs = cfg.model_timeout_secs,
        "loaded runtime configuration"
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(cfg.model_timeout_secs))
        .build()
        .context("Failed to initialize HTTP client")?;

    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        info!("starting repl mode");
        run_repl(&client, &cfg).await
    } else {
        let mut agent = Agent::new(&client, &cfg);
        let prompt = args.join(" ");
        info!(prompt_len = prompt.len(), "starting single-turn mode");
        let answer = agent.run_turn(&prompt).await?;
        println!("{}", answer.trim());
        Ok(())
    }
}
