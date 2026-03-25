use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lgtmcli", version, about = "CLI for Grafana LGTM")]
pub struct Cli {
    /// Output JSON instead of human-readable table/text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum AuthCommands {
    /// Verify that GRAFANA_URL and GRAFANA_TOKEN can access Grafana API
    Status,
}

#[derive(Subcommand)]
pub enum DatasourceCommands {
    /// List Grafana datasources
    List {
        /// Filter by datasource type (e.g. loki, prometheus, tempo, postgres)
        #[arg(long = "type", value_name = "TYPE")]
        ds_type: Option<String>,
    },
}
