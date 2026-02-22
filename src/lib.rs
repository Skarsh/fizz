pub mod agent;
pub mod config;
pub mod model;
pub mod providers;
pub mod repl;

use anyhow::{Context, Result};
use reqwest::Client;
use std::env;

use agent::Agent;
use config::Config;
use repl::run_repl;

pub async fn run() -> Result<()> {
    dotenvy::dotenv().ok();

    let cfg = Config::from_env();
    let client = Client::builder()
        .build()
        .context("Failed to initialize HTTP client")?;

    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        run_repl(&client, &cfg).await
    } else {
        let mut agent = Agent::new(&client, &cfg);
        let prompt = args.join(" ");
        let answer = agent.run_turn(&prompt).await?;
        println!("{}", answer.trim());
        Ok(())
    }
}
