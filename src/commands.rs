use log::info;

use crate::{context::Context, types::Commands};

pub(crate) async fn execute_cmd(_ctx: &Context, cmd: &Commands) -> Result<(), anyhow::Error> {
    match cmd {
        Commands::Validate { } => {
            info!("Validating Covenant deployment");
            Ok(())
        }
    }
}
