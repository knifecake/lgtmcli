use std::env;
use std::process;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::StatusCode;
use reqwest::blocking::Client;

const MIMIR_UID: &str = "internal_mimir";

#[derive(Parser)]
#[command(name = "lgtmcli", version, about = "CLI for ISFG Grafana LGTM")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Verify that GRAFANA_URL and GRAFANA_TOKEN can query Grafana datasource proxy
    Status,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Status => auth_status(),
        },
    };

    if let Err(err) = result {
        eprintln!("❌ {err}");
        process::exit(1);
    }
}

fn auth_status() -> Result<()> {
    let grafana_url = required_env("GRAFANA_URL")?;
    let grafana_token = required_env("GRAFANA_TOKEN")?;

    println!("Checking Grafana credentials against {grafana_url}...");

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("failed to build HTTP client")?;

    let endpoint = format!(
        "{}/api/datasources/proxy/uid/{}/api/v1/query",
        grafana_url.trim_end_matches('/'),
        MIMIR_UID
    );

    let response = client
        .get(&endpoint)
        .bearer_auth(&grafana_token)
        .query(&[("query", "1")])
        .send()
        .with_context(|| format!("request to {endpoint} failed"))?;

    let status = response.status();
    let body = response.text().context("failed to read response body")?;

    match status {
        s if s.is_success() => {
            println!("✅ credentials look good (HTTP {})", s.as_u16());
            println!("Successfully queried Mimir through Grafana datasource proxy ({MIMIR_UID}).");
            Ok(())
        }
        StatusCode::UNAUTHORIZED => {
            bail!("HTTP 401 Unauthorized. Check GRAFANA_TOKEN.")
        }
        StatusCode::FORBIDDEN => {
            bail!(
                "HTTP 403 Forbidden. Token is valid but lacks datasource query permission for {MIMIR_UID}."
            )
        }
        StatusCode::NOT_FOUND => {
            bail!("HTTP 404 Not Found. Verify datasource UID '{MIMIR_UID}' and GRAFANA_URL.")
        }
        _ => {
            let snippet = truncate_for_log(&body, 400);
            bail!(
                "HTTP {} from Grafana. Response: {}",
                status.as_u16(),
                snippet
            )
        }
    }
}

fn required_env(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!("missing required env var {name}"))?;
    if value.trim().is_empty() {
        bail!("env var {name} is set but empty");
    }
    Ok(value)
}

fn truncate_for_log(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    format!("{}...", &value[..max_len])
}
