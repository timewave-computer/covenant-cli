use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::CliContext;

// Inspired by https://github.com/PeggyJV/chain-registry

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AssetInfo {
    pub name: String,
    pub description: String,
    pub symbol: String,
    pub denom: String,
    pub decimals: u8,
    pub coingecko_id: String,
    pub base: String,
    pub display: String,
    pub denom_units: Vec<DenomUnit>,
    #[serde(rename = "logo_URIs")]
    pub logo_uris: LogoURIs,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct DenomUnit {
    pub denom: String,
    pub exponent: u16,
    pub aliases: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct LogoURIs {
    pub png: String,
    pub svg: String,
}

pub async fn get_chain_asset_info(
    ctx: &CliContext,
    chain_id: &str,
    asset_name: &str,
) -> Result<AssetInfo, anyhow::Error> {
    let mut json: Value = ctx
        .api_get(&format!(
            "https://chains.cosmos.directory/{}/assetlist",
            chain_id
        ))
        .await?;
    let assets_obj = json["assets"].take();
    let assets: Vec<AssetInfo> = serde_json::from_value(assets_obj).unwrap_or_default();
    let asset = assets.into_iter().find(|asset| {
        (asset.name == asset_name
            || asset.symbol == asset_name
            || asset.denom == asset_name
            || asset.display == asset_name
            || asset.base == asset_name)
            && !(asset.name.contains("(old)") || asset.symbol.contains("(old)"))
    });
    if let Some(asset) = asset {
        Ok(asset)
    } else {
        Err(anyhow::anyhow!("Asset not found"))
    }
}
