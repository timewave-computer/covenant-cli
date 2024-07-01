use anyhow::Error;
use clap::Parser;
use context::CliContext;
use dotenv::dotenv;
use types::*;

mod commands;
mod context;
mod types;
mod utils;
mod validations;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    pretty_env_logger::init();

    let ctx = CliContext::init().await?;
    let cli = Cli::parse();
    commands::execute_cmd(&ctx, &cli.command).await?;

    Ok(())
}
