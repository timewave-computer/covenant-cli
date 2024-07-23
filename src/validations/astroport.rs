use crate::{
    utils::astroport::{
        get_astroport_pair_info, get_astroport_pool_info, CustomPair, StablePair, XykPair,
    },
    verify_equals,
};
use anyhow::Error;
use astroport_liquid_pooler::msg::AstroportLiquidPoolerConfig;
use covenant_utils::PoolPriceConfig;
use log::debug;
use rust_decimal::{Decimal, RoundingStrategy};
use std::ops::Range;

use super::CovenantValidationContext;

pub async fn verify_astroport_liquid_pooler_config<'a>(
    ctx: &mut CovenantValidationContext<'a>,
    key: &'a str,
    asset_a_denom: String,
    asset_a_contribution: Decimal,
    asset_b_denom: String,
    asset_b_contribution: Decimal,
    lp_cfg: &AstroportLiquidPoolerConfig,
    pool_price_cfg: &PoolPriceConfig,
    single_side_lp_limit_pct: u32,
) -> Result<(), Error> {
    let mut key = key;
    let mut field = "pool_address";
    let pair_info = get_astroport_pair_info(&ctx.cli_context, &lp_cfg.pool_address).await?;
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
        (Some(XykPair {}), None, None, "xyk") | (None, Some(StablePair {}), None, "stable") => {
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
    let asset_a_first = lp_cfg.asset_a_denom == asset_a_denom;
    debug!("asset_a_first: {}", asset_a_first);
    let expected_asset_a = if asset_a_first {
        asset_a_denom.clone()
    } else {
        asset_b_denom.clone()
    };
    if pair_asset_a_denom == lp_cfg.asset_a_denom && expected_asset_a == lp_cfg.asset_a_denom {
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
    let expected_asset_b = if asset_a_first {
        asset_b_denom.clone()
    } else {
        asset_a_denom.clone()
    };
    if asset_b_denom == lp_cfg.asset_b_denom && expected_asset_b == lp_cfg.asset_b_denom {
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
    let pool_info = get_astroport_pool_info(&ctx.cli_context, &lp_cfg.pool_address).await?;
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
    let expected_spot_price = Decimal::try_from_i128_with_scale(
        pool_price_cfg
            .expected_spot_price
            .atomics()
            .u128()
            .try_into()?,
        18,
    )?;
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
                "expected {:.4} | current pool price {:.4} -> outside of 5% range of current pool price",
                expected_spot_price, current_pool_price
            ),
        );
    }

    field = "acceptable_price_spread";
    // Compute acceptable price spread based on expected spot price
    // Note: we should verify this based on a % provided in the metadata
    let acceptable_price_spread = Decimal::try_from_i128_with_scale(
        pool_price_cfg
            .acceptable_price_spread
            .atomics()
            .u128()
            .try_into()?,
        18,
    )?;
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

    // Ensure Single Side LP limits are within 10% for party contributions
    key = "liquid_pooler_config";
    field = "single_side_lp_limits_asset_a";
    let asset_a_limit = Decimal::from(lp_cfg.single_side_lp_limits.asset_a_limit.u128());
    debug!("{}", single_side_lp_limit_pct);
    debug!("{}", Decimal::new(single_side_lp_limit_pct as i64, 2));
    let expected_asset_a_limit = asset_a_contribution
        .checked_sub(
            asset_a_contribution
                .checked_mul(Decimal::new(single_side_lp_limit_pct as i64, 2))
                .unwrap(),
        )
        .unwrap()
        .round_dp_with_strategy(0, RoundingStrategy::AwayFromZero);
    debug!(
        "liquid_pooler_config/single_side_lp_limits_asset_a: expected {} | actual {}",
        expected_asset_a_limit, lp_cfg.single_side_lp_limits.asset_a_limit
    );
    
    verify_equals!(
        ctx,
        key,
        field,
        expected_asset_a_limit,
        asset_a_limit,
        "invalid single side lp limit: expected {} | actual {}"
    );

    field = "single_side_lp_limits_asset_b";
    let asset_b_limit = Decimal::from(lp_cfg.single_side_lp_limits.asset_b_limit.u128());
    debug!("{}", asset_b_contribution
    .checked_mul(Decimal::new(single_side_lp_limit_pct as i64, 2))
    .unwrap());
    let expected_asset_b_limit = asset_b_contribution
        .checked_sub(
            asset_b_contribution
                .checked_mul(Decimal::new(single_side_lp_limit_pct as i64, 2))
                .unwrap(),
        )
        .unwrap()
        .round_dp_with_strategy(0, RoundingStrategy::AwayFromZero);
    debug!(
        "liquid_pooler_config/single_side_lp_limits_asset_b: expected {} | actual {}",
        expected_asset_b_limit, asset_b_limit
    );
    verify_equals!(
        ctx,
        key,
        field,
        expected_asset_b_limit,
        asset_b_limit,
        "invalid single side lp limit: expected {} | actual {}"
    );

    Ok(())
}
