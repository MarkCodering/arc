use anyhow::Result;

use crate::{
    system::{driver, gpu},
    ui::output,
};

pub fn run() -> Result<()> {
    let gpu_detected = !gpu::detect()?.is_empty();
    let driver_installed = driver::detect_version()?.is_some();
    let nvidia_smi_available = driver::nvidia_smi_available();

    output::diagnostics(gpu_detected, driver_installed, nvidia_smi_available);
    Ok(())
}
