use anyhow::{Context, Result};
use reqwest::Client;
use std::io::{self, Write};

use crate::config::Config;
use crate::model::chat_once;

pub async fn run_repl(client: &Client, cfg: &Config) -> Result<()> {
    println!("fizz agent harness");
    println!("model: {}", cfg.model);
    println!("type a prompt, or 'exit' to quit");

    loop {
        print!("> ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        let read = io::stdin()
            .read_line(&mut input)
            .context("Failed to read stdin")?;
        if read == 0 {
            break;
        }

        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if prompt.eq_ignore_ascii_case("exit") || prompt.eq_ignore_ascii_case("quit") {
            break;
        }

        let answer = chat_once(client, cfg, prompt).await?;
        println!("{}\n", answer.trim());
    }

    Ok(())
}
