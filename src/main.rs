mod app;
mod cli;
mod commands;
mod grafana;
mod output;
mod time;

use std::process;

use anyhow::Result;
use clap::Parser;

use app::AppContext;
use cli::{
    AuthCommands, Cli, Commands, DatasourceCommands, LogsCommands, MetricsCommands, SqlCommands,
    TracesCommands,
};
use output::OutputMode;

fn main() {
    if let Err(err) = run() {
        eprintln!("❌ {err}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let output_mode = OutputMode::from_json_flag(cli.json);

    match cli.command {
        Commands::Auth { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                AuthCommands::Status => {
                    let result = commands::auth::status(&ctx)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
        Commands::Datasources { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                DatasourceCommands::List { ds_type } => {
                    let result = commands::datasources::list(&ctx, ds_type)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
        Commands::Logs { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                LogsCommands::Query(args) => {
                    let result = commands::logs::query(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                LogsCommands::Stats(args) => {
                    let result = commands::logs::stats(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
        Commands::Metrics { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                MetricsCommands::Query(args) => {
                    let result = commands::metrics::query(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                MetricsCommands::Range(args) => {
                    let result = commands::metrics::range(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
        Commands::Traces { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                TracesCommands::Search(args) => {
                    let result = commands::traces::search(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                TracesCommands::Get(args) => {
                    let result = commands::traces::get(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
        Commands::Sql { command } => {
            let ctx = AppContext::from_env()?;
            match command {
                SqlCommands::Query(args) => {
                    let result = commands::sql::query(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                SqlCommands::Schemas(args) => {
                    let result = commands::sql::schemas(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                SqlCommands::Tables(args) => {
                    let result = commands::sql::tables(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
                SqlCommands::Describe(args) => {
                    let result = commands::sql::describe(&ctx, args)?;
                    output::emit(output_mode, &result)?;
                }
            }
        }
    }

    Ok(())
}
