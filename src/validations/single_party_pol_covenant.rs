use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};
use rust_decimal::prelude::{One, Zero};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use single_party_pol_covenant::msg as sppc;
use std::ops::Range;

use super::{CovenantValidationContext, Validate};
use crate::utils::assets::get_chain_asset_info;
use crate::utils::astroport::{
    get_astroport_pair_info, get_astroport_pool_info, CustomPair, StablePair, XykPair,
};
use crate::utils::chain::get_chain_info;
use crate::utils::path::get_path_info;
use crate::validations::{
    get_covenant_code_ids, verify_code_id, NEUTRON_CHAIN_NAME, STRIDE_CHAIN_NAME, TRANSFER_PORT_ID,
};
use crate::{required_or_ignored, verify_equals};

/// Validate the single party POL covenant instantiation message
pub struct SinglePartyPolCovenantInstMsg(single_party_pol_covenant::msg::InstantiateMsg);

impl SinglePartyPolCovenantInstMsg {
    pub fn new(inner: single_party_pol_covenant::msg::InstantiateMsg) -> Self {
        SinglePartyPolCovenantInstMsg(inner)
    }

    pub fn into_boxed(self) -> Box<dyn Validate> {
        Box::new(self)
    }
}

#[async_trait]
impl Validate for SinglePartyPolCovenantInstMsg {
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
        let party_chain_name = ctx.covenant_party_chain_name();
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
                (
                    path_info.chain_1.connection_id.clone(),
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
            } else {
                (
                    path_info.chain_2.connection_id.clone(),
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
        let ls_share = Decimal::new(
            msg.remote_chain_splitter_config
                .ls_share
                .atomics()
                .u128()
                .try_into()?,
            18,
        );
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
        let native_share = Decimal::new(
            msg.remote_chain_splitter_config
                .native_share
                .atomics()
                .u128()
                .try_into()?,
            18,
        );
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
                &ctx.covenant_party_chain_name(),
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
                field = "pool_address";
                let pair_info =
                    get_astroport_pair_info(&ctx.cli_context, &lp_cfg.pool_address).await?;
                debug!("astroport pair info: {:?}", pair_info);
                ctx.valid_field(key, field, "verified".to_owned());

                field = "pool_pair_type";
                debug!(
                    "liquid_pooler_config/pool_pair_type: expected {:?} | actual {}",
                    pair_info.pair_type, lp_cfg.pool_pair_type
                );
                match (
                    pair_info.pair_type.xyk,
                    pair_info.pair_type.stable,
                    pair_info.pair_type.custom,
                    lp_cfg.pool_pair_type.to_string().as_ref(),
                ) {
                    (Some(XykPair {}), None, None, "xyk")
                    | (None, Some(StablePair {}), None, "stable") => {
                        ctx.valid_field(key, field, "verified".to_owned());
                    }
                    (None, None, Some(CustomPair(custom_type)), _) => {
                        if lp_cfg.pool_pair_type.to_string() == format!("custom-{}", custom_type) {
                            ctx.valid_field(key, field, "verified".to_owned());
                        } else {
                            ctx.invalid_field(key, field, "invalid pool pair type".to_owned());
                        }
                    }
                    _ => {
                        ctx.invalid_field(key, field, "invalid pool pair type".to_owned());
                    }
                }

                field = "asset_a_denom";
                let pair_asset_a = pair_info.asset_infos.first().unwrap();
                let pair_asset_a_denom = pair_asset_a
                    .native_token
                    .as_ref()
                    .map(|t| t.denom.clone())
                    .unwrap_or_default();
                debug!(
                    "liquid_pooler_config/asset_a_denom: expected {} | actual {}",
                    pair_asset_a_denom, lp_cfg.asset_a_denom
                );
                let asset_a_is_staked_asset = lp_cfg.asset_a_denom == ls_denom_on_neutron;
                let expected_asset_a = if asset_a_is_staked_asset {
                    ls_denom_on_neutron.clone()
                } else {
                    native_denom.clone()
                };
                if pair_asset_a_denom == lp_cfg.asset_a_denom
                    && expected_asset_a == lp_cfg.asset_a_denom
                {
                    ctx.valid_field(key, field, "verified".to_owned());
                } else {
                    ctx.invalid_field(
                        key,
                        field,
                        format!(
                            "invalid asset A denom '{}': should be '{}'",
                            lp_cfg.asset_a_denom, expected_asset_a
                        ),
                    );
                }

                field = "asset_b_denom";
                let asset_b = pair_info.asset_infos.last().unwrap();
                let asset_b_denom = asset_b
                    .native_token
                    .as_ref()
                    .map(|t| t.denom.clone())
                    .unwrap_or_default();
                debug!(
                    "liquid_pooler_config/asset_b_denom: expected {} | actual {}",
                    asset_b_denom, lp_cfg.asset_b_denom
                );
                let expected_asset_b = if asset_a_is_staked_asset {
                    native_denom
                } else {
                    ls_denom_on_neutron
                };
                if asset_b_denom == lp_cfg.asset_b_denom && expected_asset_b == lp_cfg.asset_b_denom
                {
                    ctx.valid_field(key, field, "verified".to_owned());
                } else {
                    ctx.invalid_field(
                        key,
                        field,
                        format!(
                            "invalid asset B denom '{}': should be '{}'",
                            lp_cfg.asset_b_denom, expected_asset_b
                        ),
                    );
                }

                // Pool price config
                key = "pool_price_config";
                let pool_info =
                    get_astroport_pool_info(&ctx.cli_context, &lp_cfg.pool_address).await?;
                debug!("astroport pool info: {:?}", pool_info);

                let asset_a_pool_amount = pool_info
                    .assets
                    .first()
                    .unwrap()
                    .amount
                    .parse::<u128>()
                    .unwrap();
                let asset_b_pool_amount = pool_info
                    .assets
                    .last()
                    .unwrap()
                    .amount
                    .parse::<u128>()
                    .unwrap();
                let current_pool_price = Decimal::from(asset_a_pool_amount)
                    .checked_div(Decimal::from(asset_b_pool_amount))
                    .unwrap_or_default();
                debug!(
                    "pool_price_config/current pool price: {} / {} = {}",
                    asset_a_pool_amount, asset_b_pool_amount, current_pool_price
                );

                field = "expected_spot_price";
                // Assume expected spot price is within 5% range of current pool price
                let expected_spot_price = Decimal::new(
                    msg.pool_price_config
                        .expected_spot_price
                        .atomics()
                        .u128()
                        .try_into()?,
                    18,
                );
                debug!(
                    "pool_price_config/expected_spot_price: {:.4}",
                    expected_spot_price
                );
                if (Range {
                    start: current_pool_price.checked_mul(Decimal::new(95, 2)).unwrap(),
                    end: current_pool_price
                        .checked_mul(Decimal::new(105, 2))
                        .unwrap(),
                })
                .contains(&expected_spot_price)
                {
                    ctx.valid_field(
                        key,
                        field,
                        "within 5% range of current pool price".to_owned(),
                    );
                } else {
                    // Just a warning for now
                    ctx.valid_field(
                        key,
                        field,
                        format!(
                            "expected_spot_price: {:.4} | current_pool_price: {:.4}\n\
                            outside of 5% range of current pool price",
                            expected_spot_price, current_pool_price
                        ),
                    );
                }

                field = "acceptable_price_spread";
                // Compute acceptable price spread based on expected spot price
                // Note: we should verify this based on a % provided in the metadata
                let acceptable_price_spread = Decimal::new(
                    msg.pool_price_config
                        .acceptable_price_spread
                        .atomics()
                        .u128()
                        .try_into()?,
                    18,
                );
                debug!(
                    "pool_price_config/acceptable_price_spread: {:.4}",
                    acceptable_price_spread
                );
                let acceptable_price_spread_pct = acceptable_price_spread
                    .checked_div(expected_spot_price)
                    .unwrap_or_default()
                    .checked_mul(Decimal::new(100, 0))
                    .unwrap();
                debug!(
                    "pool_price_config/acceptable price spread: {:.0}%",
                    acceptable_price_spread_pct
                );
                ctx.valid_field(key, field, format!("{:.0}%", acceptable_price_spread_pct));

                key = "liquid_pooler_config";
                field = "single_side_lp_limits";
                // TODO: Validate single side LP limits
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
