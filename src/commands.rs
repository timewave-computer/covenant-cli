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

const DEFAULT_SINGLE_SIDE_LP_LIMIT_PCT: u32 = 10;

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

async fn validate_covenant<'a>(
    metadata_file: &String,
    instantiation_file: &String,
    validation_context: &mut CovenantValidationContext<'a>,
) -> Result<(), anyhow::Error> {
    info!("Validating Covenant deployment");

    // Read Covenant metadata file
    let metadata: toml::Value = load_toml(metadata_file)?;
    let covenant_metadata = metadata.get("covenant").unwrap().as_table().unwrap();
    debug!("[covenant-metadata] {:?}", covenant_metadata);

    let covenant_contract = configure_context(covenant_metadata, validation_context)?;

    // Read Covenant instantiation file
    let instantiation: serde_json::Value = load_json(instantiation_file)?;

    // Match on covenant type and create wrapper to validate
    let covenant = match covenant_contract.as_ref() {
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

fn configure_context(
    covenant_metadata: &toml::map::Map<String, toml::Value>,
    validation_context: &mut CovenantValidationContext,
) -> Result<String, anyhow::Error> {
    let covenant_contract = covenant_metadata.get("contract").unwrap().as_str().unwrap();
    info!("Covenant contract: {:?}", covenant_contract);

    let covenant_party_a_chain_name = covenant_metadata
        .get("party_a_chain_name")
        .unwrap()
        .as_str()
        .unwrap();
    validation_context.set_party_a_chain_name(covenant_party_a_chain_name.to_string());

    if covenant_contract == "valence-covenant-two-party-pol"
        || covenant_contract == "valence-covenant-swap"
    {
        let covenant_party_b_chain_name = covenant_metadata
            .get("party_b_chain_name")
            .unwrap()
            .as_str()
            .unwrap();
        validation_context.set_party_b_chain_name(covenant_party_b_chain_name.to_string());
    }

    if let Some(bool_setting) = covenant_metadata.get("party_a_channel_uses_wasm_port") {
        let party_a_channel_uses_wasm_port = bool_setting.as_bool().unwrap();
        if party_a_channel_uses_wasm_port {
            validation_context.set_party_a_channel_uses_wasm_port(true);
        }
    }

    if let Some(ls_provider_setting) = covenant_metadata.get("ls_provider") {
        let ls_provider = ls_provider_setting.as_str().unwrap();
        validation_context.set_ls_provider(ls_provider.into());
    }

    if let Some(pct_setting) = covenant_metadata.get("single_side_lp_limit_pct") {
        let single_side_lp_limit_pct = pct_setting
            .as_integer()
            .unwrap();
        validation_context
            .set_single_side_lp_limit_pct(single_side_lp_limit_pct.try_into().unwrap());
    } else {
        validation_context
            .set_single_side_lp_limit_pct(DEFAULT_SINGLE_SIDE_LP_LIMIT_PCT);
    }

    Ok(covenant_contract.to_owned())
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
