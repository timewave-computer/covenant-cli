use clap::{Parser, Subcommand};

/// Covenant CLI
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Validate a Covenant deployment
    Validate {
        
    },
}
