use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};
use rust_decimal::prelude::{One, Zero};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use single_party_pol_covenant::msg as sppc;

use super::{CovenantValidationContext, Validate};
use crate::utils::assets::get_chain_asset_info;
use crate::utils::chain::get_chain_info;
use crate::utils::path::get_path_info;
use crate::validations::{
    astroport::verify_astroport_liquid_pooler_config,
    contracts::{get_covenant_code_ids, verify_code_id},
    NEUTRON_CHAIN_NAME, STRIDE_CHAIN_NAME, TRANSFER_PORT_ID,
};
use crate::{required_or_ignored, verify_equals};

/// Validate the single party POL covenant instantiation message
pub struct SinglePartyPolCovenantInstMsg(single_party_pol_covenant::msg::InstantiateMsg);

impl<'a> SinglePartyPolCovenantInstMsg {
    pub fn new(inner: single_party_pol_covenant::msg::InstantiateMsg) -> Self {
        SinglePartyPolCovenantInstMsg(inner)
    }

    pub fn into_boxed(self) -> Box<dyn Validate<'a>> {
        Box::new(self)
    }
}

#[async_trait]
impl<'a> Validate<'a> for SinglePartyPolCovenantInstMsg {
    async fn validate(&self, ctx: &mut CovenantValidationContext) -> Result<(), Error> {
        // Validate the single party POL covenant instantiation message
        let msg = &self.0;
        debug!("valence-covenant-single-party-pol: {:?}", msg);

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
                    "single_party_pol_holder",
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
                    "remote_chain_splitter_code",
                    &code_ids,
                    "remote_chain_splitter",
                    msg.contract_codes.remote_chain_splitter_code,
                );
                verify_code_id(
                    ctx,
                    "liquid_pooler_code",
                    &code_ids,
                    "astroport_liquid_pooler",
                    msg.contract_codes.liquid_pooler_code,
                );
                verify_code_id(
                    ctx,
                    "liquid_staker_code",
                    &code_ids,
                    "stride_liquid_staker",
                    msg.contract_codes.liquid_staker_code,
                );
                verify_code_id(
                    ctx,
                    "interchain_router_code",
                    &code_ids,
                    "interchain_router",
                    msg.contract_codes.interchain_router_code,
                );
            }
            Err(e) => {
                ctx.invalid(key, e.to_string());
            }
        }

        // Covenant party config
        key = "covenant_party_config";
        let party_chain_name = ctx.party_a_chain_name();
        let path_info =
            get_path_info(&ctx.cli_context, &party_chain_name, NEUTRON_CHAIN_NAME).await?;
        let (expected_connection_id, expected_h2p_channel_id, expected_p2h_channel_id) =
            if path_info.chain_1.chain_name == NEUTRON_CHAIN_NAME {
                (
                    path_info.chain_1.connection_id.clone(),
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_1.port_id == TRANSFER_PORT_ID {
                                Some(c.chain_1.channel_id.clone())
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap(),
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_2.port_id == TRANSFER_PORT_ID {
                                Some(c.chain_2.channel_id.clone())
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap(),
                )
            } else {
                (
                    path_info.chain_2.connection_id.clone(),
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_2.port_id == TRANSFER_PORT_ID {
                                Some(c.chain_2.channel_id.clone())
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap(),
                    path_info
                        .channels
                        .iter()
                        .filter_map(|c| {
                            if c.chain_1.port_id == TRANSFER_PORT_ID {
                                Some(c.chain_1.channel_id.clone())
                            } else {
                                None
                            }
                        })
                        .next()
                        .unwrap(),
                )
            };

        field = "party_chain_connection_id";
        let party_chain_connection_id = msg.covenant_party_config.party_chain_connection_id.clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_connection_id,
            party_chain_connection_id,
            "invalid connection id: expected {} | actual {}"
        );

        field = "host_to_party_chain_channel_id";
        let host_to_party_chain_channel_id = msg
            .covenant_party_config
            .host_to_party_chain_channel_id
            .clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_h2p_channel_id,
            msg.covenant_party_config.host_to_party_chain_channel_id,
            "invalid channel id: expected {} | actual {}"
        );

        field = "party_to_host_chain_channel_id";
        let party_to_host_chain_channel_id = msg
            .covenant_party_config
            .party_to_host_chain_channel_id
            .clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_p2h_channel_id,
            party_to_host_chain_channel_id,
            "invalid channel id: expected {} | actual {}"
        );

        field = "remote_chain_denom";
        let party_chain_info = get_chain_info(&ctx.cli_context, &party_chain_name).await?;
        let expected_remote_chain_denom = party_chain_info.denom.clone();
        let remote_chain_denom = msg.covenant_party_config.remote_chain_denom.clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_remote_chain_denom,
            remote_chain_denom,
            "invalid denom: expected {} | actual {}"
        );

        field = "native_denom";
        let expected_native_denom = format!(
            "ibc/{}",
            base16ct::upper::encode_string(
                Sha256::digest(
                    format!(
                        "transfer/{}/{}",
                        host_to_party_chain_channel_id, remote_chain_denom
                    )
                    .as_bytes()
                )
                .as_ref()
            )
        );
        let native_denom = msg.covenant_party_config.native_denom.clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_native_denom,
            native_denom,
            "invalid denom: expected {} | actual {}"
        );

        field = "contribution";
        if msg.covenant_party_config.contribution.denom != remote_chain_denom {
            ctx.invalid_field(
                key,
                field,
                format!(
                    "invalid denom: expected {} | actual {}",
                    remote_chain_denom, msg.covenant_party_config.contribution.denom
                ),
            );
        } else {
            let contribution_amount =
                Decimal::from(msg.covenant_party_config.contribution.amount.u128())
                    .checked_div(Decimal::from(10u128.pow(party_chain_info.decimals.into())))
                    .unwrap();
            ctx.valid_field(
                key,
                field,
                format!("{:.2} {}", contribution_amount, party_chain_info.display),
            );
        }

        // TODO: Validate the rest of the covenant party config
        field = "party_receiver_addr";
        field = "addr";
        field = "denom_to_pfm_map";
        field = "fallback_address";

        // LS info (Neutron -> Stride)
        key = "ls_info";
        let path_info =
            get_path_info(&ctx.cli_context, NEUTRON_CHAIN_NAME, STRIDE_CHAIN_NAME).await?;
        let (expected_connection_id, expected_channel_id, reverse_channel_id) =
            if path_info.chain_1.chain_name == NEUTRON_CHAIN_NAME {
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
            } else {
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
            };

        field = "ls_neutron_connection_id";
        verify_equals!(
            ctx,
            key,
            field,
            expected_connection_id,
            msg.ls_info.ls_neutron_connection_id,
            "invalid connection id: expected {} | actual {}"
        );

        field = "ls_chain_to_neutron_channel_id";
        verify_equals!(
            ctx,
            key,
            field,
            expected_channel_id,
            msg.ls_info.ls_chain_to_neutron_channel_id,
            "invalid channel id: expected {} | actual {}"
        );

        field = "ls_denom";
        let ls_denom = msg.ls_info.ls_denom.clone();
        let asset_info =
            get_chain_asset_info(&ctx.cli_context, STRIDE_CHAIN_NAME, &ls_denom).await?;
        if asset_info.base == ls_denom {
            ctx.valid_field(key, field, "verified".to_owned());
        } else {
            ctx.invalid_field(key, field, format!("could not verify denom: {}", ls_denom));
        }

        field = "ls_denom_on_neutron";
        let expected_ls_denom_on_neutron = format!(
            "ibc/{}",
            base16ct::upper::encode_string(
                Sha256::digest(
                    format!("transfer/{}/{}", reverse_channel_id, msg.ls_info.ls_denom).as_bytes()
                )
                .as_ref()
            )
        );
        let ls_denom_on_neutron = msg.ls_info.ls_denom_on_neutron.clone();
        verify_equals!(
            ctx,
            key,
            field,
            expected_ls_denom_on_neutron,
            ls_denom_on_neutron,
            "invalid denom: expected {} | actual {}"
        );

        // Remote chain splitter
        key = "remote_chain_splitter_config";
        field = "connection_id";
        verify_equals!(
            ctx,
            key,
            field,
            party_chain_connection_id,
            msg.remote_chain_splitter_config.connection_id,
            "invalid connection id: expected {} | actual {}"
        );

        field = "channel_id";
        verify_equals!(
            ctx,
            key,
            field,
            host_to_party_chain_channel_id,
            msg.remote_chain_splitter_config.channel_id,
            "invalid channel id: expected {} | actual {}"
        );

        field = "denom";
        verify_equals!(
            ctx,
            key,
            field,
            remote_chain_denom,
            msg.remote_chain_splitter_config.denom,
            "invalid denom: expected {} | actual {}"
        );

        field = "amount";
        verify_equals!(
            ctx,
            key,
            field,
            msg.covenant_party_config.contribution.amount,
            msg.remote_chain_splitter_config.amount,
            "invalid amount: expected {} | actual {}"
        );

        field = "ls_share";
        let ls_share = Decimal::try_from_i128_with_scale(
            msg.remote_chain_splitter_config
                .ls_share
                .atomics()
                .u128()
                .try_into()?,
            18,
        )?;
        if ls_share >= Decimal::zero() && ls_share <= Decimal::one() {
            ctx.valid_field(key, field, "verified".to_owned());
        } else {
            ctx.invalid_field(
                key,
                field,
                "invalid share: should be between 0 and 1".to_owned(),
            );
        }

        field = "native_share";
        let native_share = Decimal::try_from_i128_with_scale(
            msg.remote_chain_splitter_config
                .native_share
                .atomics()
                .u128()
                .try_into()?,
            18,
        )?;
        if native_share >= Decimal::zero() && native_share <= Decimal::one() {
            ctx.valid_field(key, field, "verified".to_owned());
        } else {
            ctx.invalid_field(
                key,
                field,
                "invalid share: should be between 0 and 1".to_owned(),
            );
        }

        // ls_share + native_share should sum up to 1
        if ls_share + native_share == Decimal::one() {
            ctx.valid_field(key, "ls_share + native_share", "verified".to_owned());
        } else {
            ctx.invalid_field(
                key,
                "ls_share + native_share",
                "invalid share: should sum up to 1".to_owned(),
            );
        }

        // LP forwarder config
        key = "lp_forwarder_config";
        if let sppc::CovenantPartyConfig::Interchain(lp_fwd_cfg) = &msg.lp_forwarder_config {
            field = "party_receiver_addr";
            required_or_ignored!(ctx, key, field, &lp_fwd_cfg.party_receiver_addr);

            field = "addr";
            required_or_ignored!(ctx, key, field, &lp_fwd_cfg.addr);

            field = "host_to_party_chain_channel_id";
            required_or_ignored!(ctx, key, field, &lp_fwd_cfg.host_to_party_chain_channel_id);

            field = "native_denom";
            required_or_ignored!(ctx, key, field, &lp_fwd_cfg.native_denom);

            field = "remote_chain_denom";
            verify_equals!(
                ctx,
                key,
                field,
                remote_chain_denom,
                lp_fwd_cfg.remote_chain_denom,
                "invalid denom: expected {} | actual {}"
            );

            field = "party_chain_connection_id";
            verify_equals!(
                ctx,
                key,
                field,
                party_chain_connection_id,
                lp_fwd_cfg.party_chain_connection_id,
                "invalid connection id: expected {} | actual {}"
            );

            field = "party_to_host_chain_channel_id";
            verify_equals!(
                ctx,
                key,
                field,
                party_to_host_chain_channel_id,
                lp_fwd_cfg.party_to_host_chain_channel_id,
                "invalid channel id: expected {} | actual {}"
            );

            field = "contribution";
            if lp_fwd_cfg.contribution.denom != remote_chain_denom {
                ctx.invalid_field(
                    key,
                    field,
                    format!(
                        "invalid denom: expected {} | actual {}",
                        remote_chain_denom, lp_fwd_cfg.contribution.denom
                    ),
                );
            } else if Decimal::from(lp_fwd_cfg.contribution.amount.u128())
                != Decimal::from(msg.covenant_party_config.contribution.amount.u128())
                    .checked_mul(native_share)
                    .unwrap()
            {
                ctx.invalid_field(
                    key,
                    field,
                    "invalid amount: should be equal to native_share * contribution amount"
                        .to_owned(),
                );
            } else {
                let contribution_amount = Decimal::from(lp_fwd_cfg.contribution.amount.u128())
                    .checked_div(Decimal::from(10u128.pow(party_chain_info.decimals.into())))
                    .unwrap();
                ctx.valid_field(
                    key,
                    field,
                    format!("{:.2} {}", contribution_amount, party_chain_info.display),
                );
            }
        } else {
            ctx.invalid(
                key,
                "Invalit covenant party config: should be an Interchain party config.".to_owned(),
            );
        }

        // LS forwarder config
        key = "ls_forwarder_config";
        if let sppc::CovenantPartyConfig::Interchain(ls_fwd_cfg) = &msg.ls_forwarder_config {
            field = "party_receiver_addr";
            required_or_ignored!(ctx, key, field, &ls_fwd_cfg.party_receiver_addr);

            field = "addr";
            required_or_ignored!(ctx, key, field, &ls_fwd_cfg.addr);

            field = "host_to_party_chain_channel_id";
            required_or_ignored!(ctx, key, field, &ls_fwd_cfg.host_to_party_chain_channel_id);

            field = "native_denom";
            required_or_ignored!(ctx, key, field, &ls_fwd_cfg.native_denom);

            field = "remote_chain_denom";
            verify_equals!(
                ctx,
                key,
                field,
                remote_chain_denom,
                ls_fwd_cfg.remote_chain_denom,
                "invalid denom: expected {} | actual {}"
            );

            let ls_path_info = get_path_info(
                &ctx.cli_context,
                &ctx.party_a_chain_name(),
                STRIDE_CHAIN_NAME,
            )
            .await?;
            let (expected_ls_connection_id, expected_ls_p2h_channel_id) =
                if ls_path_info.chain_1.chain_name == STRIDE_CHAIN_NAME {
                    (
                        ls_path_info.chain_1.connection_id.clone(),
                        ls_path_info
                            .channels
                            .iter()
                            .filter_map(|c| {
                                if c.chain_2.port_id == TRANSFER_PORT_ID {
                                    Some(c.chain_2.channel_id.clone())
                                } else {
                                    None
                                }
                            })
                            .next()
                            .unwrap(),
                    )
                } else {
                    (
                        ls_path_info.chain_2.connection_id.clone(),
                        ls_path_info
                            .channels
                            .iter()
                            .filter_map(|c| {
                                if c.chain_1.port_id == TRANSFER_PORT_ID {
                                    Some(c.chain_1.channel_id.clone())
                                } else {
                                    None
                                }
                            })
                            .next()
                            .unwrap(),
                    )
                };

            field = "party_chain_connection_id";
            verify_equals!(
                ctx,
                key,
                field,
                expected_ls_connection_id,
                ls_fwd_cfg.party_chain_connection_id,
                "invalid connection id: expected {} | actual {}"
            );

            field = "party_to_host_chain_channel_id";
            verify_equals!(
                ctx,
                key,
                field,
                expected_ls_p2h_channel_id,
                ls_fwd_cfg.party_to_host_chain_channel_id,
                "invalid channel id: expected {} | actual {}"
            );

            field = "contribution";
            if ls_fwd_cfg.contribution.denom != remote_chain_denom {
                ctx.invalid_field(
                    key,
                    field,
                    format!(
                        "invalid denom: expected {} | actual {}",
                        remote_chain_denom, ls_fwd_cfg.contribution.denom
                    ),
                );
            } else if Decimal::from(ls_fwd_cfg.contribution.amount.u128())
                != Decimal::from(msg.covenant_party_config.contribution.amount.u128())
                    .checked_mul(ls_share)
                    .unwrap()
            {
                ctx.invalid_field(
                    key,
                    field,
                    "invalid amount: should be equal to ls_share * contribution amount".to_owned(),
                );
            } else {
                let contribution_amount = Decimal::from(ls_fwd_cfg.contribution.amount.u128())
                    .checked_div(Decimal::from(10u128.pow(party_chain_info.decimals.into())))
                    .unwrap();
                ctx.valid_field(
                    key,
                    field,
                    format!("{:.2} {}", contribution_amount, party_chain_info.display),
                );
            }
        } else {
            ctx.invalid(
                key,
                "Invalit covenant party config: should be an Interchain party config.".to_owned(),
            );
        }

        // Liquid pooler config
        key = "liquid_pooler_config";
        match &msg.liquid_pooler_config {
            sppc::LiquidPoolerConfig::Astroport(lp_cfg) => {
                verify_astroport_liquid_pooler_config(
                    ctx,
                    key,
                    native_denom,
                    ls_denom_on_neutron,
                    lp_cfg,
                    &msg.pool_price_config,
                )
                .await?;
            }
            sppc::LiquidPoolerConfig::Osmosis(_lp_cfg) => {
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
