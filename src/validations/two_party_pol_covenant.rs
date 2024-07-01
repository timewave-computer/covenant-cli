use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};

use super::{CovenantValidationContext, Validate};

/// Validate the two party POL covenant instantiation message
pub struct TwoPartyPolCovenantInstMsg(two_party_pol_covenant::msg::InstantiateMsg);

impl TwoPartyPolCovenantInstMsg {
    pub fn new(inner: two_party_pol_covenant::msg::InstantiateMsg) -> Self {
        TwoPartyPolCovenantInstMsg(inner)
    }

    pub fn into_boxed(self) -> Box<dyn Validate> {
        Box::new(self)
    }
}

#[async_trait]
impl Validate for TwoPartyPolCovenantInstMsg {
    async fn validate(&self, _ctx: &mut CovenantValidationContext) -> Result<(), Error> {
        // Validate the two party POL covenant instantiation message
        let msg = &self.0;
        debug!("valence-covenant-two-party-pol: {:?}", msg);

        info!("Processing covenant {:?}", msg.label);

        Ok(())
    }
}
