use clap::{Args, Parser, Subcommand, ValueEnum};

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
    Logs {
        #[command(subcommand)]
        command: LogsCommands,
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

#[derive(Subcommand)]
pub enum LogsCommands {
    /// Run a LogQL query over a time range
    Query(LogsQueryArgs),
}

#[derive(Debug, Clone, Args)]
pub struct LogsQueryArgs {
    /// LogQL expression
    pub query: String,

    /// Logs datasource UID (Grafana datasource UID)
    #[arg(long = "ds", value_name = "UID")]
    pub datasource_uid: String,

    /// Relative range from now (e.g. 15m, 1h, 24h)
    #[arg(long, value_name = "DURATION")]
    pub since: Option<String>,

    /// RFC3339 timestamp for range start (must be used with --to)
    #[arg(long, value_name = "TIMESTAMP")]
    pub from: Option<String>,

    /// RFC3339 timestamp for range end (must be used with --from)
    #[arg(long, value_name = "TIMESTAMP")]
    pub to: Option<String>,

    /// Maximum number of log lines to return
    #[arg(long, default_value_t = 100)]
    pub limit: u32,

    /// Loki query direction
    #[arg(long, value_enum, default_value_t = LogDirectionArg::Backward)]
    pub direction: LogDirectionArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogDirectionArg {
    Backward,
    Forward,
}

impl LogDirectionArg {
    pub fn as_loki_param(self) -> &'static str {
        match self {
            Self::Backward => "backward",
            Self::Forward => "forward",
        }
    }
}
