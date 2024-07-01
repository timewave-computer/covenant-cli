use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::context::CliContext;

const NEUTRON_RPC_URL: &str = "https://rest-kralum.neutron-1.neutron.org";
// const COIN_REGISTRY_CONTRACT_ADDRESS: &str =
//     "neutron1jzzv6r5uckwd64n6qan3suzker0kct5w565f6529zjyumfcx96kqtcswn3";
// const FACTORY_CONTRACT_CODE_ID: &str =
//     "neutron1hptk0k5kng7hjy35vmh009qd5m6l33609nypgf2yc6nqnewduqasxplt4e";
const COSMWASM_CONTRACT_API: &str = "cosmwasm/wasm/v1/contract";
const COSMWASM_SMART_QUERY: &str = "smart";
// const RESULT_LIMIT: usize = 30;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct NativeTokentInfo {
    pub denom: String,
    pub decimals: u8,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct PairInfo {
    pub contract_addr: String,
    pub liquidity_token: String,
    pub pair_type: PairType,
    pub asset_infos: Vec<AssetInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct PoolInfo {
    pub assets: Vec<PoolAssetInfo>,
    pub total_share: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct PairType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xyk: Option<XykPair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable: Option<StablePair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<CustomPair>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct XykPair {}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct StablePair {}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CustomPair(pub String);

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct PoolAssetInfo {
    pub amount: String,
    pub info: AssetInfo,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AssetInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Token>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_token: Option<NativeToken>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct NativeToken {
    pub denom: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Token {
    pub contract_addr: String,
}

pub async fn get_astroport_pair_info(
    ctx: &CliContext,
    pool_addr: &str,
) -> Result<PairInfo, anyhow::Error> {
    let base_url = format!(
        "{}/{}/{}/{}",
        NEUTRON_RPC_URL, COSMWASM_CONTRACT_API, pool_addr, COSMWASM_SMART_QUERY,
    );

    let smart_query = URL_SAFE.encode(json!({ "pair": {} }).to_string());
    let mut json: Value = ctx
        .api_get(&format!("{}/{}", base_url, smart_query))
        .await?;
    let pair_obj = json["data"].take();

    let pair: PairInfo = serde_json::from_value(pair_obj).unwrap_or_default();
    Ok(pair)
}

pub async fn get_astroport_pool_info(
    ctx: &CliContext,
    pool_addr: &str,
) -> Result<PoolInfo, anyhow::Error> {
    let base_url = format!(
        "{}/{}/{}/{}",
        NEUTRON_RPC_URL, COSMWASM_CONTRACT_API, pool_addr, COSMWASM_SMART_QUERY,
    );

    let smart_query = URL_SAFE.encode(json!({ "pool": {} }).to_string());
    let mut json: Value = ctx
        .api_get(&format!("{}/{}", base_url, smart_query))
        .await?;
    let pool_obj = json["data"].take();

    let pool: PoolInfo = serde_json::from_value(pool_obj).unwrap_or_default();
    Ok(pool)
}
