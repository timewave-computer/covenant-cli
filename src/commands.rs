use anyhow::Context;
use log::{debug, info};

use crate::types::Commands;

pub(crate) async fn execute_cmd(
    _ctx: &crate::Context,
    cmd: &Commands,
) -> Result<(), anyhow::Error> {
    match cmd {
        Commands::Validate {
            metadata_file,
            instantiation_file,
        } => validate_covenant(metadata_file, instantiation_file),
    }
}

fn validate_covenant(
    metadata_file: &String,
    instantiation_file: &String,
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

    // Read Covenant instantiation file
    let instantiation: serde_json::Value = load_json(instantiation_file)?;

    match covenant_contract {
        "valence-covenant-single-party-pol" => {
            let msg: single_party_pol_covenant::msg::InstantiateMsg =
                serde_json::from_value(instantiation)
                    .with_context(|| "failed loading single party POL covenant")?;
            validate_single_party_pol_covenant(&msg, covenant_metadata)
        }
        "valence-covenant-swap" => {
            let msg: swap_covenant::msg::InstantiateMsg = serde_json::from_value(instantiation)
                .with_context(|| "failed loading swap covenant")?;
            validate_swap_covenant(&msg, covenant_metadata)
        }
        "valence-covenant-two-party-pol" => {
            let msg: two_party_pol_covenant::msg::InstantiateMsg =
                serde_json::from_value(instantiation)
                    .with_context(|| "failed loading two party POL covenant")?;
            validate_two_party_pol_covenant(&msg, covenant_metadata)
        }
        _ => Err(anyhow::anyhow!("Unsupported covenant contract")),
    }
}

fn load_toml(metadata_file: &String) -> Result<toml::Value, anyhow::Error> {
    toml::from_str(&std::fs::read_to_string(metadata_file)?)
        .with_context(|| "failed loading TOML file")
}

fn load_json(instantiation_file: &String) -> Result<serde_json::Value, anyhow::Error> {
    serde_json::from_str(&std::fs::read_to_string(instantiation_file)?)
        .with_context(|| "failed loading JSON file")
}

fn validate_swap_covenant(
    msg: &swap_covenant::msg::InstantiateMsg,
    _metadata: &toml::Table,
) -> Result<(), anyhow::Error> {
    debug!("InstantiateMsg: {:?}", msg);
    info!("Processing covenant {:?}", msg.label);
    todo!()
}

fn validate_single_party_pol_covenant(
    msg: &single_party_pol_covenant::msg::InstantiateMsg,
    _metadata: &toml::Table,
) -> Result<(), anyhow::Error> {
    debug!("InstantiateMsg: {:?}", msg);
    info!("Processing covenant {:?}", msg.label);
    todo!()
}

fn validate_two_party_pol_covenant(
    msg: &two_party_pol_covenant::msg::InstantiateMsg,
    _metadata: &toml::Table,
) -> Result<(), anyhow::Error> {
    debug!("InstantiateMsg: {:?}", msg);
    info!("Processing covenant {:?}", msg.label);
    // println!("✅ Label: {:?}", msg.label);
    // println!("⛔️ Error: {:?}", msg.label);
    todo!()
}
