use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use two_party_pol_covenant::msg as tppc;

use super::{CovenantValidationContext, Validate};
use crate::utils::assets::get_chain_asset_info;
use crate::utils::chain::get_chain_info;
use crate::utils::path::{get_path_info, IBCPath};
use crate::utils::validate_party_address;
use crate::validations::astroport::verify_astroport_liquid_pooler_config;
use crate::validations::neutron::verify_expiration;
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
        let mut field = "label";
        if msg.label.is_empty() {
            ctx.invalid_field(key, field, "required".to_owned());
        } else {
            ctx.valid_field(key, field, "valid".to_owned());
        }

        // Contract Codes
        key = "contract_codes";
        verify_two_party_pol_covenant_code_ids(ctx, key, &msg.contract_codes).await?;

        // Covenant type
        key = "covenant";
        field = "covenant_type";
        ctx.valid_field(key, field, "verified".to_owned());
        // match msg.covenant_type {
        //     valence_two_party_pol_holder::msg::CovenantType::Side => {}
        //     valence_two_party_pol_holder::msg::CovenantType::Share => {}
        // }

        // Party shares
        if msg.party_a_share + msg.party_b_share == cosmwasm_std::Decimal::one() {
            ctx.valid_field(key, "party_a_share", "verified".to_owned());
            ctx.valid_field(key, "party_b_share", "verified".to_owned());
        } else {
            ctx.invalid_field(
                key,
                "party_b_share",
                "invalid share: sum of shares should be 1.0".to_owned(),
            );
            ctx.invalid_field(
                key,
                "party_b_share",
                "invalid share: sum of shares should be 1.0".to_owned(),
            );
        }

        // Deposit deadline
        field = "deposit_deadline";
        verify_expiration(ctx, key, field, msg.deposit_deadline).await?;

        // Lockup config
        field = "lockup_config";
        verify_expiration(ctx, key, field, msg.lockup_config).await?;

        // Lockup config should be later than deposit deadline
        // (this should work as Expiration implements PartialOrd)
        if msg.lockup_config <= msg.deposit_deadline {
            ctx.invalid_field(
                key,
                field,
                "invalid lockup config: should be later than deposit deadline".to_owned(),
            );
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
                    msg.party_a_config.get_native_denom(),
                    Decimal::from(get_party_contribution(&msg.party_a_config).u128()),
                    msg.party_b_config.get_native_denom(),
                    Decimal::from(get_party_contribution(&msg.party_b_config).u128()),
                    lp_cfg,
                    &msg.pool_price_config,
                    ctx.single_side_lp_limit_pct,
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

        // Splits
        key = "splits";
        field = "";
        let party_a_denom = msg.party_a_config.get_native_denom();
        let party_a_receiver = msg.party_a_config.get_final_receiver_address();
        let party_b_denom = msg.party_b_config.get_native_denom();
        let party_b_receiver = msg.party_b_config.get_final_receiver_address();
        match &msg.splits.keys().cloned().collect::<Vec<_>>()[..] {
            [denom_1, denom_2]
                if (denom_1 == &party_a_denom && denom_2 == &party_b_denom)
                    || (denom_2 == &party_a_denom && denom_1 == &party_b_denom) =>
            {
                let denom_1_receivers = &msg.splits[denom_1].receivers;
                let denom_2_receivers = &msg.splits[denom_2].receivers;
                if denom_1_receivers.contains_key(&party_a_receiver)
                    && denom_1_receivers.contains_key(&party_b_receiver)
                    && denom_1_receivers[&party_a_receiver] == cosmwasm_std::Decimal::one()
                    && denom_1_receivers
                        .iter()
                        .map(|r| r.1)
                        .sum::<cosmwasm_std::Decimal>()
                        == cosmwasm_std::Decimal::one()
                    && denom_2_receivers.contains_key(&party_a_receiver)
                    && denom_2_receivers.contains_key(&party_b_receiver)
                    && denom_2_receivers[&party_b_receiver] == cosmwasm_std::Decimal::one()
                    && denom_2_receivers
                        .iter()
                        .map(|r| r.1)
                        .sum::<cosmwasm_std::Decimal>()
                        == cosmwasm_std::Decimal::one()
                {
                    ctx.valid_field(key, field, "verified".to_owned());
                } else {
                    ctx.invalid_field(
                        key,
                        field,
                        "invalid splits: sum of splits should be 1.0".to_owned(),
                    );
                }
            }
            _ => {
                ctx.invalid_field(key, field, "invalid splits: unexpected denoms".to_owned());
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
    match party_config {
        tppc::CovenantPartyConfig::Native(native_party) => {
            let mut field = "native_denom";
            // verify_chain_denom(ctx, key, field, party_chain_name, native_party).await?;
            let party_chain_info = get_chain_info(&ctx.cli_context, party_chain_name).await?;
            let party_chain_denom = party_chain_info.denom.clone();
            let mut party_base_denom = party_chain_denom.clone();
            let mut party_base_denom_decimals = party_chain_info.decimals;
            let native_denom = native_party.native_denom.clone();

            if native_denom == party_chain_denom {
                // Simple case: denom is the chain's native token
                verify_equals!(
                    ctx,
                    key,
                    field,
                    party_chain_denom,
                    native_denom,
                    "invalid denom: expected {} | actual {}"
                );
            } else if native_denom.starts_with("ibc/") {
                // IBC denom
                match get_chain_asset_info(&ctx.cli_context, NEUTRON_CHAIN_NAME, &native_denom)
                    .await
                {
                    Ok(asset_info) => {
                        party_base_denom = asset_info
                            .denom_units
                            .iter()
                            .find_map(|d| {
                                if d.exponent == 0 {
                                    d.aliases.as_ref().and_then(|a| {
                                        a.iter().find_map(|a| {
                                            if a.starts_with('u') {
                                                Some(a.clone())
                                            } else {
                                                None
                                            }
                                        })
                                    })
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| asset_info.base.clone());
                        party_base_denom_decimals = asset_info.decimals;
                        verify_equals!(
                            ctx,
                            key,
                            field,
                            asset_info.base,
                            native_denom,
                            "invalid denom: expected {} | actual {}"
                        );
                    }
                    Err(_) => {
                        ctx.invalid_field(key, field, "unknown denom".to_owned());
                    }
                }
            } else {
                ctx.invalid_field(key, field, "unknown denom".to_owned());
            }

            field = "contribution";
            if native_party.contribution.denom != party_base_denom
                && native_party.contribution.denom != native_denom
            {
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
                    format!("{:.2} {}", contribution_amount, party_chain_info.display),
                );
            }

            field = "party_receiver_addr";
            validate_party_address(ctx, key, field, native_party.party_receiver_addr.as_str());
            field = "addr";
            validate_party_address(ctx, key, field, native_party.addr.as_str());
        }
        tppc::CovenantPartyConfig::Interchain(interchain_party) => {
            let path_info =
                get_path_info(&ctx.cli_context, party_chain_name, NEUTRON_CHAIN_NAME).await?;
            debug!(
                "party_a_uses_wasm_port: {}",
                ctx.party_a_channel_uses_wasm_port
            );
            let (expected_connection_id, expected_h2p_channel_id, expected_p2h_channel_id) =
                get_path_connection_and_channels(&path_info, ctx.party_a_channel_uses_wasm_port);

            let mut field = "party_chain_connection_id";
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

            field = "remote_chain_denom";
            let party_chain_info = get_chain_info(&ctx.cli_context, party_chain_name).await?;
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
                match get_chain_asset_info(&ctx.cli_context, party_chain_name, &remote_chain_denom)
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
                            if let Ok(asset_info) =
                                get_chain_asset_info(&ctx.cli_context, party_chain_name, asset_name)
                                    .await
                            {
                                if (asset_name == asset_info.denom)
                                    || (asset_name == asset_info.display)
                                {
                                    asset_name.clone_into(&mut remote_chain_denom);
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
            remote_chain_denom.clone_from(&interchain_party.remote_chain_denom);
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
            validate_party_address(ctx, key, field, interchain_party.party_receiver_addr.as_str());
            field = "addr";
            validate_party_address(ctx, key, field, interchain_party.addr.as_str());

            //TODO: Validate the rest of the covenant party config
            // field = "denom_to_pfm_map";
            // field = "fallback_address";
        }
    }

    Ok(())
}

async fn verify_two_party_pol_covenant_code_ids<'a>(
    ctx: &mut CovenantValidationContext<'a>,
    key: &'a str,
    contract_code_ids: &tppc::CovenantContractCodeIds,
) -> Result<(), Error> {
    match get_covenant_code_ids("v0.1.0".to_owned()).await {
        Ok(code_ids) => {
            verify_code_id(
                ctx,
                "ibc_forwarder_code",
                &code_ids,
                "ibc_forwarder",
                contract_code_ids.ibc_forwarder_code,
            );
            verify_code_id(
                ctx,
                "holder_code",
                &code_ids,
                "two_party_pol_holder",
                contract_code_ids.holder_code,
            );
            verify_code_id(
                ctx,
                "clock_code",
                &code_ids,
                "clock",
                contract_code_ids.clock_code,
            );
            verify_code_id(
                ctx,
                "interchain_router_code",
                &code_ids,
                "interchain_router",
                contract_code_ids.interchain_router_code,
            );
            verify_code_id(
                ctx,
                "native_router_code",
                &code_ids,
                "native_router",
                contract_code_ids.native_router_code,
            );
            verify_code_id(
                ctx,
                "liquid_pooler_code",
                &code_ids,
                "astroport_liquid_pooler",
                contract_code_ids.liquid_pooler_code,
            );
        }
        Err(e) => {
            ctx.invalid(key, e.to_string());
        }
    }
    Ok(())
}

fn get_path_connection_and_channels(
    path_info: &IBCPath,
    channel_uses_wasm_port: bool,
) -> (String, String, String) {
    if path_info.chain_1.chain_name == NEUTRON_CHAIN_NAME {
        path_info
            .channels
            .iter()
            .filter_map(|c| {
                if c.chain_1.port_id == TRANSFER_PORT_ID
                    && ((channel_uses_wasm_port && c.chain_2.port_id.starts_with("wasm."))
                        || (!channel_uses_wasm_port && c.chain_2.port_id == TRANSFER_PORT_ID))
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
                    && ((channel_uses_wasm_port && c.chain_1.port_id.starts_with("wasm."))
                        || c.chain_1.port_id == TRANSFER_PORT_ID)
                {
                    Some((
                        path_info.chain_2.connection_id.clone(),
                        c.chain_2.channel_id.clone(),
                        c.chain_1.channel_id.clone(),
                    ))
                } else {
                    None
                }
            })
            .next()
            .unwrap()
    }
}

fn get_party_contribution(cfg: &tppc::CovenantPartyConfig) -> cosmwasm_std::Uint128 {
    match cfg {
        tppc::CovenantPartyConfig::Interchain(interchain) => interchain.contribution.amount,
        tppc::CovenantPartyConfig::Native(native) => native.contribution.amount,
    }
}
