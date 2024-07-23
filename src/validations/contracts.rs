use anyhow::{Context, Error};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use super::CovenantValidationContext;

pub async fn get_covenant_code_ids(version: String) -> Result<HashMap<String, u64>, Error> {
    let content = reqwest::get(format!(
        "https://github.com/timewave-computer/covenants/releases/download/{}/contract_code_ids.txt",
        version
    ))
    .await
    .with_context(|| "failed fetching contract code ids from covenants release")?
    .text()
    .await?;

    let mut code_ids = HashMap::new();
    let reader = BufReader::new(content.as_bytes());
    for line in reader.lines() {
        let line = line.with_context(|| "failed reading line from contract_code_ids.txt file")?;
        let parts: Vec<&str> = line.split_ascii_whitespace().collect();
        if parts.len() == 2 {
            let contract_name = parts[0].trim().replace("valence_", "").replace(".wasm", "");
            let code_id = parts[1].trim();
            code_ids.insert(contract_name, code_id.parse::<u64>().unwrap());
        } else {
            return Err(anyhow::anyhow!(
                "invalid line in contract_code_ids.txt file"
            ));
        }
    }
    Ok(code_ids)
}

pub fn verify_code_id(
    ctx: &mut CovenantValidationContext,
    field: &'static str,
    code_ids: &HashMap<String, u64>,
    contract_name: &str,
    code_id: u64,
) {
    if code_ids.contains_key(contract_name) {
        if code_ids.get(contract_name).unwrap() == &code_id {
            ctx.valid_field("contract_codes", field, "verified".to_owned());
        } else {
            ctx.invalid_field("contract_codes", field, "invalid code id".to_owned());
        }
    } else {
        ctx.invalid(
            "contract_codes",
            format!("unknown contract name {}", contract_name),
        );
    }
}
