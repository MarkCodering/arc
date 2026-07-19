use anyhow::{Context, Result};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};

use crate::system::driver::DriverFlavor;

pub fn confirm_install() -> Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue with this installation plan?")
        .default(false)
        .interact()
        .context("could not read installation confirmation")
}

pub fn select_driver_flavor() -> Result<DriverFlavor> {
    let options = [
        "Open kernel modules (Turing and newer)",
        "Proprietary kernel modules (Maxwell, Pascal, Volta)",
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("GPU generation could not be determined. Which driver should be installed?")
        .items(&options)
        .default(0)
        .interact()
        .context("could not read the selected driver flavor")?;
    Ok(if selection == 0 {
        DriverFlavor::Open
    } else {
        DriverFlavor::Proprietary
    })
}

pub fn confirm_uninstall() -> Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue with this uninstall plan?")
        .default(false)
        .interact()
        .context("could not read uninstall confirmation")
}
