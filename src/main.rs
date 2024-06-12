use anyhow::Error;
use clap::Parser;
use context::Context;
use dotenv::dotenv;
use types::*;

mod commands;
mod context;
mod types;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    pretty_env_logger::init();

    let ctx = Context::init().await?;
    let cli = Cli::parse();
    let _res = commands::execute_cmd(&ctx, &cli.command).await?;

    Ok(())
}
