use anyhow::Error;
use bech32::{primitives::decode::CheckedHrpstring, Bech32};

use crate::validations::CovenantValidationContext;

pub mod assets;
pub mod astroport;
pub mod chain;
pub mod neutron;
pub mod path;

pub fn validate_party_address<'a>(ctx: &mut CovenantValidationContext<'a>,
    key: &'a str, field : &'a str, address: &str) {
    // Decode the Bech32 address
    match validate_bech32_address(address) {
        Ok(_) => {
            // Note: no check is done on the HRP (Human Readable Part) for now
            ctx.valid_field(key, field, "valid Bech32 address".to_owned());
        },
        Err(_) => {
            ctx.invalid_field(key, field, "Invalid Bech32 address".to_owned());
        }
    }
}

fn validate_bech32_address(address: &str) -> Result<(), Error> {
    let _ = CheckedHrpstring::new::<Bech32>(address)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

#[test]
fn test_validate_bech32_address() {
    validate_bech32_address("cosmos1ayw8xtxkty5cfzx44z6vxpevmtudg2n3f4etcq").unwrap();
    validate_bech32_address("neutron1ayw8xtxkty5cfzx44z6vxpevmtudg2n3d2sfz8").unwrap();
}