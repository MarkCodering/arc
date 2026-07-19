use anyhow::Result;

use crate::{
    system::{driver, gpu, os},
    ui::output,
};

pub fn run() -> Result<()> {
    let os = os::detect()?;
    let gpu = gpu::detect()?;
    let driver = driver::detect_version()?;

    output::system_status(&os, gpu.as_deref(), driver.as_deref());
    Ok(())
}
