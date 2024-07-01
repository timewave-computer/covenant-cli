use anyhow::{Context, Error};
use async_trait::async_trait;
use serde::Serialize;
// use single_party_pol_covenant::msg as sppc;
use crate::context::CliContext;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

mod single_party_pol_covenant;
mod swap_covenant;
mod two_party_pol_covenant;

pub use single_party_pol_covenant::SinglePartyPolCovenantInstMsg;
pub use swap_covenant::SwapCovenantInstMsg;
pub use two_party_pol_covenant::TwoPartyPolCovenantInstMsg;

const NEUTRON_CHAIN_NAME: &str = "neutron";
const STRIDE_CHAIN_NAME: &str = "stride";
const TRANSFER_PORT_ID: &str = "transfer";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(default)]
pub struct CovenantValidationContext {
    #[serde(skip)]
    cli_context: CliContext,
    covenant_party_chain_name: String,
    checks: HashMap<&'static str, Vec<String>>,
    errors: HashMap<&'static str, Vec<String>>,
}

impl CovenantValidationContext {
    pub fn covenant_party_chain_name(&self) -> String {
        self.covenant_party_chain_name.clone()
    }

    pub fn set_covenant_party_chain_name(&mut self, party: String) {
        self.covenant_party_chain_name = party;
    }

    pub fn checks(&self) -> &HashMap<&'static str, Vec<String>> {
        &self.checks
    }

    pub fn errors(&self) -> &HashMap<&'static str, Vec<String>> {
        &self.errors
    }

    #[allow(unused)]
    pub fn valid(&mut self, key: &'static str, message: String) {
        self.checks.entry(key).or_default().push(message);
    }

    pub fn valid_field(&mut self, key: &'static str, field: &'static str, message: String) {
        self.checks
            .entry(key)
            .or_default()
            .push(format!("{}: {}", field, message));
    }

    pub fn invalid(&mut self, key: &'static str, message: String) {
        self.errors.entry(key).or_default().push(message);
    }

    pub fn invalid_field(&mut self, key: &'static str, field: &'static str, message: String) {
        self.errors
            .entry(key)
            .or_default()
            .push(format!("{}: {}", field, message));
    }
}

#[macro_export]
macro_rules! required_or_ignored {
    ($ctx:expr, $key:expr, $field:expr, $value:expr) => {
        if $value.is_empty() {
            $ctx.invalid_field($key, $field, "required".to_owned());
        } else {
            $ctx.valid_field($key, $field, "ignored".to_owned());
        }
    };
}

#[macro_export]
macro_rules! verify_equals {
    ($ctx:expr, $key:expr, $field:expr, $expected:expr, $actual:expr, $error_fmt:expr) => {
        if $actual == $expected {
            $ctx.valid_field($key, $field, "verified".to_owned());
        } else {
            $ctx.invalid_field($key, $field, format!($error_fmt, $expected, $actual));
        }
    };
}

#[async_trait]
pub trait Validate {
    async fn validate(&self, ctx: &mut CovenantValidationContext) -> Result<(), Error>;
}

async fn get_covenant_code_ids(version: String) -> Result<HashMap<String, u64>, Error> {
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

fn verify_code_id(
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
