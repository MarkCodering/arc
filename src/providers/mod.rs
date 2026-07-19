use anyhow::Result;

use crate::model::environment::ProviderStatus;

pub mod nvidia;

/// Shared inspection contract implemented by each GPU vendor integration.
pub trait AcceleratorProvider {
    fn inspect(&self) -> Result<ProviderStatus>;
}

/// Providers registered here automatically participate in shared inspection commands.
pub fn registered() -> Vec<Box<dyn AcceleratorProvider>> {
    vec![Box::new(nvidia::NvidiaProvider)]
}
