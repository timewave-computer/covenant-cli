use anyhow::Error;
use cw_utils::Expiration;
use std::time::{SystemTime, UNIX_EPOCH};

use super::CovenantValidationContext;
use crate::utils::neutron::get_latest_block;

pub async fn verify_expiration<'a>(
    ctx: &mut CovenantValidationContext<'a>,
    key: &'a str,
    field: &'a str,
    deadline: Expiration,
) -> Result<(), Error> {
    match deadline {
        Expiration::AtHeight(height) => {
            let cur_block = get_latest_block(&ctx.cli_context).await?;
            if (height as u128) > cur_block {
                ctx.valid_field(key, field, "verified".to_owned());
            } else {
                ctx.invalid_field(
                    key,
                    field,
                    "invalid block height: should be in the future".to_owned(),
                );
            }
        }
        Expiration::AtTime(timestamp) => {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
            if timestamp.seconds() > now.as_secs() {
                ctx.valid_field(key, field, "verified".to_owned());
            } else {
                ctx.invalid_field(
                    key,
                    field,
                    "invalid timestamp: should be in the future".to_owned(),
                );
            }
        }
        Expiration::Never {} => {
            ctx.valid_field(key, field, "verified (note: never expires)".to_owned());
        }
    }
    Ok(())
}
