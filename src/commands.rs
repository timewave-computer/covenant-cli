use anyhow::Context;
use itertools::Itertools;
use log::{debug, error, info};

use crate::{
    types::Commands,
    validations::{
        CovenantValidationContext, SinglePartyPolCovenantInstMsg, SwapCovenantInstMsg,
        TwoPartyPolCovenantInstMsg,
    },
};

pub(crate) async fn execute_cmd(
    _ctx: &crate::CliContext,
    cmd: &Commands,
) -> Result<(), anyhow::Error> {
    match cmd {
        Commands::Validate {
            metadata_file,
            instantiation_file,
        } => {
            let mut ctx = CovenantValidationContext::default();
            validate_covenant(metadata_file, instantiation_file, &mut ctx).await?;
            render_markdown_table(&ctx);
            if ctx.has_errors() {
                let err_msg = "Covenant validation failed";
                error!("{}", err_msg);
                anyhow::bail!(err_msg);
            }
            Ok(())
        }
    }
}

async fn validate_covenant(
    metadata_file: &String,
    instantiation_file: &String,
    validation_context: &mut CovenantValidationContext,
) -> Result<(), anyhow::Error> {
    info!("Validating Covenant deployment");

    // Read Covenant metadata file
    let metadata: toml::Value = load_toml(metadata_file)?;
    let covenant_metadata = metadata.get("covenant").unwrap().as_table().unwrap();
    debug!("[covenant-metadata] {:?}", covenant_metadata);

    let covenant_contract = covenant_metadata
        .get("covenant_contract")
        .unwrap()
        .as_str()
        .unwrap();
    info!("Covenant contract: {:?}", covenant_contract);

    let covenant_party_chain_id = covenant_metadata
        .get("covenant_party_chain_name")
        .unwrap()
        .as_str()
        .unwrap();
    validation_context.set_covenant_party_chain_name(covenant_party_chain_id.to_string());

    // Read Covenant instantiation file
    let instantiation: serde_json::Value = load_json(instantiation_file)?;

    // Match on covenant type and create wrapper to validate
    let covenant = match covenant_contract {
        "valence-covenant-single-party-pol" => SinglePartyPolCovenantInstMsg::new(
            serde_json::from_value(instantiation)
                .with_context(|| "failed loading single party POL covenant")?,
        )
        .into_boxed(),
        "valence-covenant-swap" => SwapCovenantInstMsg::new(
            serde_json::from_value(instantiation)
                .with_context(|| "failed loading swap covenant")?,
        )
        .into_boxed(),
        "valence-covenant-two-party-pol" => TwoPartyPolCovenantInstMsg::new(
            serde_json::from_value(instantiation)
                .with_context(|| "failed loading two party POL covenant")?,
        )
        .into_boxed(),
        _ => panic!("Unsupported covenant contract"),
    };

    // Validate the covenant
    covenant
        .validate(validation_context)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

fn load_toml(metadata_file: &String) -> Result<toml::Value, anyhow::Error> {
    toml::from_str(&std::fs::read_to_string(metadata_file)?)
        .with_context(|| "failed loading TOML file")
}

fn load_json(instantiation_file: &String) -> Result<serde_json::Value, anyhow::Error> {
    serde_json::from_str(&std::fs::read_to_string(instantiation_file)?)
        .with_context(|| "failed loading JSON file")
}

fn render_markdown_table(ctx: &CovenantValidationContext) {
    let mut is_first_key_msg = true;
    println!("| Key | Field | Message | Status |\n| :--- | :--- | :--- | :---: |");
    for (key, messages) in ctx.checks().iter().sorted_by_key(|x| x.0) {
        for message in messages {
            let parts = message.split(": ").collect::<Vec<&str>>();
            println!(
                "| {} | {} | {} | ✅ |",
                if is_first_key_msg { key } else { "" },
                parts.first().unwrap(),
                parts.last().unwrap().replace('|', "&#124;")
            );
            if is_first_key_msg {
                is_first_key_msg = false;
            }
        }
        is_first_key_msg = true;
    }
    for (key, messages) in ctx.errors().iter().sorted_by_key(|x| x.0) {
        for message in messages {
            let parts = message.split(": ").collect::<Vec<&str>>();
            println!(
                "| {} | {} | {} | ⛔️ |",
                if is_first_key_msg { key } else { "" },
                parts.first().unwrap(),
                parts.last().unwrap().replace('|', "&#124;")
            );
            if is_first_key_msg {
                is_first_key_msg = false;
            }
        }
        is_first_key_msg = true;
    }
}
