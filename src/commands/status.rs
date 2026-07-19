use anyhow::Result;

use crate::{
    system::{driver, gpu, os},
    ui::output,
};

pub fn run() -> Result<()> {
    let os = os::detect()?;
    let gpus = gpu::detect()?;
    let driver = driver::detect_version()?;
    let gpu_summary = (!gpus.is_empty()).then(|| {
        gpus.iter()
            .map(|gpu| gpu.name.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    });

    output::system_status(&os, gpu_summary.as_deref(), driver.as_deref());
    Ok(())
}
