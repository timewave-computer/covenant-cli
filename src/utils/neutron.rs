use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::CliContext;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct BlockHeader {
    pub chain_id: String,
    pub height: String,
    pub time: String,
}

pub async fn get_latest_block(ctx: &CliContext) -> Result<u128, anyhow::Error> {
    let mut json: Value = ctx
        .api_get("https://neutron-tw-rpc.polkachu.com:443/block")
        .await?;
    let header_obj = json["result"]["block"]["header"].take();
    let header: BlockHeader = serde_json::from_value(header_obj).unwrap_or_default();

    header
        .height
        .parse::<u128>()
        .map_err(|e| anyhow::anyhow!("Error parsing block height: {:?}", e))
}
