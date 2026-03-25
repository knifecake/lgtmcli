use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "lgtmcli",
    version,
    about = "CLI for Grafana LGTM",
    after_help = "EXAMPLES:\n  lgtmcli auth status\n  lgtmcli ds list --type loki\n  lgtmcli logs query '{service=\"api\"}' --ds loki-prod --since 1h\n  lgtmcli logs stats 'quantile_over_time(0.95, ({host=\"app-1\", role=\"web\"} |= \"gunicorn.access\" | json | unwrap server_time_ms)[1m])' --ds loki-prod --since 1h --step 1m\n  lgtmcli metrics range 'rate(http_requests_total[5m])' --ds mimir-prod --since 1h --step 30s\n  lgtmcli traces search '{ status = error }' --ds tempo-prod --since 1h --json\n\nTIPS:\n  - Output defaults to table; use --json for scripts\n  - Use --ds with datasource UID from `lgtmcli ds list`"
)]
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
    Metrics {
        #[command(subcommand)]
        command: MetricsCommands,
    },
    Traces {
        #[command(subcommand)]
        command: TracesCommands,
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
#[command(
    after_help = "EXAMPLES:\n  lgtmcli logs query '{service=\"api\"}' --ds loki-prod --since 1h\n  lgtmcli logs query '{service=\"api\"} |= \"error\"' --ds loki-prod --since 1h\n  lgtmcli logs stats 'rate({service=\"api\"}[5m])' --ds loki-prod --since 1h --step 1m\n  lgtmcli logs stats 'avg by () (quantile_over_time(0.95, ({host=\"app-1\", role=\"web\"} |= \"gunicorn.access\" | json | unwrap server_time_ms)[1m]))' --ds loki-prod --since 1h --step 1m"
)]
pub enum LogsCommands {
    /// Run a LogQL query over a time range
    Query(LogsQueryArgs),
    /// Run a LogQL stats query over a time range (metrics-style output)
    Stats(LogsStatsArgs),
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

#[derive(Debug, Clone, Args)]
pub struct LogsStatsArgs {
    /// LogQL metric/stats expression
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

    /// Query resolution step (e.g. 30s, 1m)
    #[arg(long, default_value = "1m", value_name = "DURATION")]
    pub step: String,
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

#[derive(Subcommand)]
pub enum MetricsCommands {
    /// Run an instant PromQL query
    Query(MetricsQueryArgs),
    /// Run a range PromQL query
    Range(MetricsRangeArgs),
}

#[derive(Debug, Clone, Args)]
pub struct MetricsQueryArgs {
    /// PromQL expression
    pub query: String,

    /// Metrics datasource UID (Grafana datasource UID)
    #[arg(long = "ds", value_name = "UID")]
    pub datasource_uid: String,

    /// RFC3339 timestamp for instant query evaluation (defaults to now)
    #[arg(long, value_name = "TIMESTAMP")]
    pub time: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct MetricsRangeArgs {
    /// PromQL expression
    pub query: String,

    /// Metrics datasource UID (Grafana datasource UID)
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

    /// Query resolution step (e.g. 15s, 1m)
    #[arg(long, default_value = "30s", value_name = "DURATION")]
    pub step: String,
}

#[derive(Subcommand)]
pub enum TracesCommands {
    /// Search traces with a TraceQL query over a time range
    Search(TracesSearchArgs),
    /// Fetch a trace by trace ID
    Get(TraceGetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TracesSearchArgs {
    /// TraceQL expression
    pub query: String,

    /// Traces datasource UID (Grafana datasource UID)
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

    /// Maximum number of traces to return
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
}

#[derive(Debug, Clone, Args)]
pub struct TraceGetArgs {
    /// Trace ID
    pub trace_id: String,

    /// Traces datasource UID (Grafana datasource UID)
    #[arg(long = "ds", value_name = "UID")]
    pub datasource_uid: String,
}
