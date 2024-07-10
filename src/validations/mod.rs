use crate::context::CliContext;
use anyhow::Error;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;

mod astroport;
mod contracts;
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
pub struct CovenantValidationContext<'a> {
    #[serde(skip)]
    cli_context: CliContext,
    party_a_chain_name: String,
    party_a_channel_uses_wasm_port: bool,
    party_b_chain_name: String,
    checks: HashMap<&'a str, Vec<String>>,
    errors: HashMap<&'a str, Vec<String>>,
}

impl<'a> CovenantValidationContext<'a> {
    pub fn party_a_chain_name(&self) -> String {
        self.party_a_chain_name.clone()
    }

    pub fn set_party_a_chain_name(&mut self, party: String) {
        self.party_a_chain_name = party;
    }

    pub fn party_a_channel_uses_wasm_port(&self) -> bool {
        self.party_a_channel_uses_wasm_port
    }

    pub(crate) fn set_party_a_channel_uses_wasm_port(&mut self, value: bool) {
        self.party_a_channel_uses_wasm_port = value;
    }

    pub fn party_b_chain_name(&self) -> String {
        self.party_b_chain_name.clone()
    }

    pub fn set_party_b_chain_name(&mut self, party: String) {
        self.party_b_chain_name = party;
    }

    pub fn checks(&self) -> &HashMap<&'a str, Vec<String>> {
        &self.checks
    }

    pub fn errors(&self) -> &HashMap<&'a str, Vec<String>> {
        &self.errors
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    #[allow(unused)]
    pub fn valid(&mut self, key: &'a str, message: String) {
        self.checks.entry(key).or_default().push(message);
    }

    pub fn valid_field(&mut self, key: &'a str, field: &'a str, message: String) {
        self.checks
            .entry(key)
            .or_default()
            .push(format!("{}: {}", field, message));
    }

    pub fn invalid(&mut self, key: &'a str, message: String) {
        self.errors.entry(key).or_default().push(message);
    }

    pub fn invalid_field(&mut self, key: &'a str, field: &'a str, message: String) {
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
pub trait Validate<'a> {
    async fn validate(&self, ctx: &mut CovenantValidationContext<'a>) -> Result<(), Error>;
}
