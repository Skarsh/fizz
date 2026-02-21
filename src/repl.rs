use anyhow::{Context, Result};
use reqwest::Client;
use std::io::{self, Write};

use crate::agent::Agent;
use crate::config::Config;
use crate::model::Message;

pub async fn run_repl(client: &Client, cfg: &Config) -> Result<()> {
    let mut agent = Agent::new(client, cfg);

    println!("fizz agent harness");
    println!("model: {}", cfg.model);
    println!(
        "type a prompt, '/history' to inspect memory, '/reset' to clear memory, or 'exit' to quit"
    );

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
        if prompt.eq_ignore_ascii_case("/reset") {
            agent.reset();
            println!("conversation reset\n");
            continue;
        }
        if prompt.eq_ignore_ascii_case("/history") {
            print_history(agent.history());
            continue;
        }

        let answer = agent.run_turn(prompt).await?;
        println!("{}\n", answer.trim());
    }

    Ok(())
}

fn print_history(history: &[Message]) {
    if history.is_empty() {
        println!("(history is empty)\n");
        return;
    }

    for (idx, msg) in history.iter().enumerate() {
        println!("[{}] {}: {}", idx, msg.role.as_str(), msg.content);
    }
    println!();
}
