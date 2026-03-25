use std::env;
use std::process;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::Deserialize;

const REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Parser)]
#[command(name = "lgtmcli", version, about = "CLI for Grafana LGTM")]
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
    #[command(visible_alias = "ds")]
    Datasources {
        #[command(subcommand)]
        command: DatasourceCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Verify that GRAFANA_URL and GRAFANA_TOKEN can access Grafana API
    Status,
}

#[derive(Subcommand)]
enum DatasourceCommands {
    /// List Grafana datasources
    List {
        /// Filter by datasource type (e.g. loki, prometheus, tempo, postgres)
        #[arg(long = "type", value_name = "TYPE")]
        ds_type: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct DataSource {
    id: i64,
    uid: String,
    name: String,
    #[serde(rename = "type")]
    ds_type: String,
    #[serde(rename = "isDefault", default)]
    is_default: bool,
}

struct GrafanaConfig {
    base_url: String,
    token: String,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Status => auth_status(),
        },
        Commands::Datasources { command } => match command {
            DatasourceCommands::List { ds_type } => datasources_list(ds_type),
        },
    };

    if let Err(err) = result {
        eprintln!("❌ {err}");
        process::exit(1);
    }
}

fn auth_status() -> Result<()> {
    let cfg = grafana_config_from_env()?;
    let client = build_http_client()?;
    let datasources = fetch_datasources(&client, &cfg)?;

    println!("✅ credentials look good");
    println!("Successfully reached Grafana API at {}", cfg.base_url);
    println!("Visible datasources: {}", datasources.len());

    Ok(())
}

fn datasources_list(ds_type: Option<String>) -> Result<()> {
    let cfg = grafana_config_from_env()?;
    let client = build_http_client()?;
    let mut datasources = fetch_datasources(&client, &cfg)?;

    if let Some(filter) = ds_type.as_deref() {
        datasources.retain(|ds| ds.ds_type.eq_ignore_ascii_case(filter));
    }

    datasources.sort_by_key(|ds| {
        (
            ds.ds_type.to_ascii_lowercase(),
            ds.name.to_ascii_lowercase(),
        )
    });

    if datasources.is_empty() {
        if let Some(filter) = ds_type {
            println!("No datasources found for --type '{filter}'.");
        } else {
            println!("No datasources found.");
        }
        return Ok(());
    }

    println!("ID\tUID\tTYPE\tNAME\tDEFAULT");
    for ds in datasources {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            ds.id,
            ds.uid,
            ds.ds_type,
            ds.name,
            if ds.is_default { "yes" } else { "no" }
        );
    }

    Ok(())
}

fn fetch_datasources(client: &Client, cfg: &GrafanaConfig) -> Result<Vec<DataSource>> {
    let endpoint = format!("{}/api/datasources", cfg.base_url.trim_end_matches('/'));

    let response = client
        .get(&endpoint)
        .bearer_auth(&cfg.token)
        .send()
        .with_context(|| format!("request to {endpoint} failed"))?;

    let status = response.status();
    let body = response.text().context("failed to read response body")?;

    ensure_grafana_success(status, &body, "calling Grafana datasources API")?;

    let datasources: Vec<DataSource> =
        serde_json::from_str(&body).context("failed to parse datasource list JSON")?;

    Ok(datasources)
}

fn build_http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .context("failed to build HTTP client")
}

fn grafana_config_from_env() -> Result<GrafanaConfig> {
    Ok(GrafanaConfig {
        base_url: required_env("GRAFANA_URL")?,
        token: required_env("GRAFANA_TOKEN")?,
    })
}

fn ensure_grafana_success(status: StatusCode, body: &str, action: &str) -> Result<()> {
    match status {
        s if s.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => {
            bail!("HTTP 401 Unauthorized while {action}. Check GRAFANA_TOKEN.")
        }
        StatusCode::FORBIDDEN => {
            bail!("HTTP 403 Forbidden while {action}. Token lacks required Grafana permissions.")
        }
        StatusCode::NOT_FOUND => bail!("HTTP 404 Not Found while {action}. Check GRAFANA_URL."),
        _ => {
            let snippet = truncate_for_log(body, 400);
            bail!(
                "HTTP {} while {action}. Response: {}",
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
