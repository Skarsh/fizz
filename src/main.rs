mod config;
mod model;
mod providers;
mod repl;

use anyhow::{Context, Result};
use reqwest::Client;
use std::env;

use config::Config;
use model::chat_once;
use repl::run_repl;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::from_env();
    let client = Client::builder()
        .build()
        .context("Failed to initialize HTTP client")?;

    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        run_repl(&client, &cfg).await
    } else {
        let prompt = args.join(" ");
        let answer = chat_once(&client, &cfg, &prompt).await?;
        println!("{}", answer.trim());
        Ok(())
    }
}
