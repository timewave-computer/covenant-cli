use anyhow::Error;
use async_trait::async_trait;
use log::{debug, info};

use super::{CovenantValidationContext, Validate};

/// Validate the swap covenant instantiation message
pub struct SwapCovenantInstMsg(swap_covenant::msg::InstantiateMsg);

impl SwapCovenantInstMsg {
    pub fn new(inner: swap_covenant::msg::InstantiateMsg) -> Self {
        SwapCovenantInstMsg(inner)
    }

    pub fn into_boxed(self) -> Box<dyn Validate> {
        Box::new(self)
    }
}

#[async_trait]
impl Validate for SwapCovenantInstMsg {
    async fn validate(&self, _ctx: &mut CovenantValidationContext) -> Result<(), Error> {
        // Validate the two party POL covenant instantiation message
        let msg = &self.0;
        debug!("valence-covenant-swap: {:?}", msg);

        info!("Processing covenant {:?}", msg.label);

        Ok(())
    }
}
