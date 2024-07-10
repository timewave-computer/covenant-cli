use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use two_party_pol_covenant::msg as tppc;

use super::{CovenantValidationContext, Validate};
use crate::utils::assets::get_chain_asset_info;
use crate::utils::chain::get_chain_info;
use crate::utils::path::get_path_info;
use crate::validations::astroport::verify_astroport_liquid_pooler_config;
use crate::validations::{
    contracts::{get_covenant_code_ids, verify_code_id},
    NEUTRON_CHAIN_NAME, TRANSFER_PORT_ID,
};
use crate::verify_equals;

/// Validate the two party POL covenant instantiation message
pub struct TwoPartyPolCovenantInstMsg(two_party_pol_covenant::msg::InstantiateMsg);

impl<'a> TwoPartyPolCovenantInstMsg {
    pub fn new(inner: two_party_pol_covenant::msg::InstantiateMsg) -> Self {
        TwoPartyPolCovenantInstMsg(inner)
    }

    pub fn into_boxed(self) -> Box<dyn Validate<'a>> {
        Box::new(self)
    }
}

#[async_trait]
impl<'a> Validate<'a> for TwoPartyPolCovenantInstMsg {
    async fn validate(&self, ctx: &mut CovenantValidationContext) -> Result<(), Error> {
        // Validate the two party POL covenant instantiation message
        let msg = &self.0;
        debug!("valence-covenant-two-party-pol: {:?}", msg);

        info!("Processing covenant {:?}", msg.label);

        // Covenant label
        let mut key = "covenant";
        let mut field = "";
        if msg.label.is_empty() {
            ctx.invalid_field(key, "label", "required".to_owned());
        } else {
            ctx.valid_field(key, "label", "valid".to_owned());
        }

        // Contract Codes
        key = "contract_codes";
        match get_covenant_code_ids("v0.1.0".to_owned()).await {
            Ok(code_ids) => {
                verify_code_id(
                    ctx,
                    "ibc_forwarder_code",
                    &code_ids,
                    "ibc_forwarder",
                    msg.contract_codes.ibc_forwarder_code,
                );
                verify_code_id(
                    ctx,
                    "holder_code",
                    &code_ids,
                    "two_party_pol_holder",
                    msg.contract_codes.holder_code,
                );
                verify_code_id(
                    ctx,
                    "clock_code",
                    &code_ids,
                    "clock",
                    msg.contract_codes.clock_code,
                );
                verify_code_id(
                    ctx,
                    "interchain_router_code",
                    &code_ids,
                    "interchain_router",
                    msg.contract_codes.interchain_router_code,
                );
                verify_code_id(
                    ctx,
                    "native_router_code",
                    &code_ids,
                    "native_router",
                    msg.contract_codes.native_router_code,
                );
                verify_code_id(
                    ctx,
                    "liquid_pooler_code",
                    &code_ids,
                    "astroport_liquid_pooler",
                    msg.contract_codes.liquid_pooler_code,
                );
            }
            Err(e) => {
                ctx.invalid(key, e.to_string());
            }
        }

        // Party A config
        key = "party_a_config";
        let party_a_chain_name = ctx.party_a_chain_name();
        verify_party_config(
            ctx,
            key,
            &party_a_chain_name,
            &msg.party_a_config,
            ctx.party_a_channel_uses_wasm_port,
        )
        .await?;

        // Party B config
        key = "party_b_config";
        let party_b_chain_name = ctx.party_b_chain_name();
        verify_party_config(ctx, key, &party_b_chain_name, &msg.party_b_config, false).await?;

        // Liquid pooler config
        key = "liquid_pooler_config";
        match &msg.liquid_pooler_config {
            tppc::LiquidPoolerConfig::Astroport(lp_cfg) => {
                verify_astroport_liquid_pooler_config(
                    ctx,
                    key,
                    party_native_denom(&msg.party_a_config),
                    party_native_denom(&msg.party_b_config),
                    lp_cfg,
                    &msg.pool_price_config,
                )
                .await?;
            }
            tppc::LiquidPoolerConfig::Osmosis(_lp_cfg) => {
                ctx.invalid(
                    key,
                    "Osmosis liquid pooler config: validation logic not yet implemented."
                        .to_owned(),
                );

                // Pool price config
                key = "pool_price_config";
                ctx.invalid(
                    key,
                    "Osmosis pool price config: validation logic not yet implemented.".to_owned(),
                );
            }
        }

        Ok(())
    }
}

async fn verify_party_config<'a>(
    ctx: &mut CovenantValidationContext<'a>,
    key: &'a str,
    party_chain_name: &str,
    party_config: &tppc::CovenantPartyConfig,
    _party_channel_uses_wasm_port: bool,
) -> Result<(), Error> {
    let mut field = "";
    match party_config {
        tppc::CovenantPartyConfig::Native(native_party) => {
            field = "native_denom";
            let party_chain_info = get_chain_info(&ctx.cli_context, &party_chain_name).await?;
            let party_chain_denom = party_chain_info.denom.clone();
            let mut party_base_denom = party_chain_denom.clone();
            let mut party_base_denom_decimals = party_chain_info.decimals;
            let mut native_denom = native_party.native_denom.clone();
            if native_denom == party_chain_denom {
                verify_equals!(
                    ctx,
                    key,
                    field,
                    party_chain_denom,
                    native_denom,
                    "invalid denom: expected {} | actual {}"
                );
            } else {
                // Native denom is not the chain's native token
                match get_chain_asset_info(&ctx.cli_context, &party_chain_name, &native_denom).await
                {
                    Ok(asset_info) => {
                        party_base_denom = asset_info.base;
                        party_base_denom_decimals = asset_info.decimals;
                        verify_equals!(
                            ctx,
                            key,
                            field,
                            asset_info.denom,
                            native_denom,
                            "invalid denom: expected {} | actual {}"
                        );
                    }
                    Err(_) => {
                        let mut verified = false;
                        if native_denom.starts_with('u') {
                            let native_denom_tmp = native_denom.clone();
                            let asset_name = native_denom_tmp.strip_prefix('u').unwrap();
                            if let Ok(asset_info) = get_chain_asset_info(
                                &ctx.cli_context,
                                &party_chain_name,
                                asset_name,
                            )
                            .await
                            {
                                if (asset_name == asset_info.denom)
                                    || (asset_name == asset_info.display)
                                {
                                    native_denom = asset_name.to_owned();
                                    party_base_denom = asset_info.base;
                                    party_base_denom_decimals = asset_info.decimals;
                                    ctx.valid_field(
                                        key,
                                        field,
                                        format!("verified (with denom '{}')", asset_name),
                                    );
                                    verified = true;
                                }
                            }
                        }
                        if !verified {
                            ctx.invalid_field(key, field, "unknown denom".to_owned());
                        }
                    }
                }
            }
            field = "contribution";
            // remote_chain_denom = interchain_party.remote_chain_denom.clone();
            if native_party.contribution.denom != party_base_denom {
                ctx.invalid_field(
                    key,
                    field,
                    format!(
                        "invalid denom: expected {} | actual {}",
                        party_base_denom, native_party.contribution.denom
                    ),
                );
            } else {
                let contribution_amount = Decimal::from(native_party.contribution.amount.u128())
                    .checked_div(Decimal::from(10u128.pow(party_base_denom_decimals.into())))
                    .unwrap();
                ctx.valid_field(
                    key,
                    field,
                    format!("{:.2} {}", contribution_amount, party_base_denom),
                );
            }
            field = "party_receiver_addr";
            field = "addr";
        }
        tppc::CovenantPartyConfig::Interchain(interchain_party) => {
            let path_info =
                get_path_info(&ctx.cli_context, &party_chain_name, NEUTRON_CHAIN_NAME).await?;
            debug!(
                "party_a_uses_wasm_port: {}",
                ctx.party_a_channel_uses_wasm_port
            );
            let (expected_connection_id, expected_h2p_channel_id, expected_p2h_channel_id) =
                if path_info.chain_1.chain_name == NEUTRON_CHAIN_NAME {
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_1.port_id == TRANSFER_PORT_ID
                                && ((ctx.party_a_channel_uses_wasm_port
                                    && c.chain_2.port_id.starts_with("wasm."))
                                    || (!ctx.party_a_channel_uses_wasm_port
                                        && c.chain_2.port_id == TRANSFER_PORT_ID))
                            {
                                Some((
                                    path_info.chain_1.connection_id.clone(),
                                    c.chain_1.channel_id.clone(),
                                    c.chain_2.channel_id.clone(),
                                ))
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap()
                } else {
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_2.port_id == TRANSFER_PORT_ID
                                && ((ctx.party_a_channel_uses_wasm_port
                                    && c.chain_1.port_id.starts_with("wasm."))
                                    || c.chain_1.port_id == TRANSFER_PORT_ID)
                            {
                                Some((
                                    path_info.chain_1.connection_id.clone(),
                                    c.chain_2.channel_id.clone(),
                                    c.chain_1.channel_id.clone(),
                                ))
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap()
                };

            field = "party_chain_connection_id";
            let party_chain_connection_id = interchain_party.party_chain_connection_id.clone();
            verify_equals!(
                ctx,
                key,
                field,
                expected_connection_id,
                party_chain_connection_id,
                "invalid connection id: expected {} | actual {}"
            );

            field = "host_to_party_chain_channel_id";
            let host_to_party_chain_channel_id =
                interchain_party.host_to_party_chain_channel_id.clone();
            verify_equals!(
                ctx,
                key,
                field,
                expected_h2p_channel_id,
                interchain_party.host_to_party_chain_channel_id,
                "invalid channel id: expected {} | actual {}"
            );

            field = "party_to_host_chain_channel_id";
            let party_to_host_chain_channel_id =
                interchain_party.party_to_host_chain_channel_id.clone();
            verify_equals!(
                ctx,
                key,
                field,
                expected_p2h_channel_id,
                party_to_host_chain_channel_id,
                "invalid channel id: expected {} | actual {}"
            );

            // TODO: fix logic to handle another asset than the chain's native asset
            field = "remote_chain_denom";
            let party_chain_info = get_chain_info(&ctx.cli_context, &party_chain_name).await?;
            let party_chain_denom = party_chain_info.denom.clone();
            let mut party_base_denom = party_chain_denom.clone();
            let mut party_base_denom_decimals = party_chain_info.decimals;
            let mut remote_chain_denom = interchain_party.remote_chain_denom.clone();
            if remote_chain_denom == party_chain_denom {
                verify_equals!(
                    ctx,
                    key,
                    field,
                    party_chain_denom,
                    remote_chain_denom,
                    "invalid denom: expected {} | actual {}"
                );
            } else {
                // Remote denom is not the chain's native token
                match get_chain_asset_info(&ctx.cli_context, &party_chain_name, &remote_chain_denom)
                    .await
                {
                    Ok(asset_info) => {
                        party_base_denom = asset_info.base;
                        party_base_denom_decimals = asset_info.decimals;
                        verify_equals!(
                            ctx,
                            key,
                            field,
                            asset_info.denom,
                            remote_chain_denom,
                            "invalid denom: expected {} | actual {}"
                        );
                    }
                    Err(_) => {
                        let mut verified = false;
                        if remote_chain_denom.starts_with('u') {
                            let remote_chain_denom_tmp = remote_chain_denom.clone();
                            let asset_name = remote_chain_denom_tmp.strip_prefix('u').unwrap();
                            if let Ok(asset_info) = get_chain_asset_info(
                                &ctx.cli_context,
                                &party_chain_name,
                                asset_name,
                            )
                            .await
                            {
                                if (asset_name == asset_info.denom)
                                    || (asset_name == asset_info.display)
                                {
                                    remote_chain_denom = asset_name.to_owned();
                                    party_base_denom = asset_info.base;
                                    party_base_denom_decimals = asset_info.decimals;
                                    ctx.valid_field(
                                        key,
                                        field,
                                        format!("verified (with denom '{}')", asset_name),
                                    );
                                    verified = true;
                                }
                            }
                        }
                        if !verified {
                            ctx.invalid_field(key, field, "unknown denom".to_owned());
                        }
                    }
                }
            }

            field = "native_denom";
            let expected_native_denom = format!(
                "ibc/{}",
                base16ct::upper::encode_string(
                    Sha256::digest(
                        format!(
                            "transfer/{}/{}",
                            host_to_party_chain_channel_id, party_base_denom
                        )
                        .as_bytes()
                    )
                    .as_ref()
                )
            );
            let native_denom = interchain_party.native_denom.clone();
            verify_equals!(
                ctx,
                key,
                field,
                expected_native_denom,
                native_denom,
                "invalid denom: expected {} | actual {}"
            );

            field = "contribution";
            remote_chain_denom = interchain_party.remote_chain_denom.clone();
            debug!("party_base_denom_decimals: {}", party_base_denom_decimals);
            if interchain_party.contribution.denom != remote_chain_denom {
                ctx.invalid_field(
                    key,
                    field,
                    format!(
                        "invalid denom: expected {} | actual {}",
                        remote_chain_denom, interchain_party.contribution.denom
                    ),
                );
            } else {
                let contribution_amount =
                    Decimal::from(interchain_party.contribution.amount.u128())
                        .checked_div(Decimal::from(10u128.pow(party_base_denom_decimals.into())))
                        .unwrap();
                ctx.valid_field(
                    key,
                    field,
                    format!("{:.2} {}", contribution_amount, remote_chain_denom),
                );
            }

            field = "party_receiver_addr";
            field = "addr";
            field = "denom_to_pfm_map";
            field = "fallback_address";
        }
    }

    Ok(())
}

fn party_native_denom(party_config: &tppc::CovenantPartyConfig) -> String {
    match party_config {
        tppc::CovenantPartyConfig::Native(native_party) => native_party.native_denom.clone(),
        tppc::CovenantPartyConfig::Interchain(interchain_party) => {
            interchain_party.native_denom.clone()
        }
    }
}
