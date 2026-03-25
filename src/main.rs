mod app;
mod cli;
mod commands;
mod grafana;
mod output;

use std::process;

use anyhow::Result;
use clap::Parser;

use app::AppContext;
use cli::{AuthCommands, Cli, Commands, DatasourceCommands, LogsCommands};
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
            }
        }
    }

    Ok(())
}
