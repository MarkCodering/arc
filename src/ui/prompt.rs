use anyhow::{Context, Result};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};

#[derive(Clone, Copy, Debug)]
pub enum UsageProfile {
    AiMachineLearning,
    CudaDevelopment,
}

impl UsageProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::AiMachineLearning => "AI / Machine Learning",
            Self::CudaDevelopment => "CUDA Development",
        }
    }
}

pub fn select_usage_profile() -> Result<UsageProfile> {
    let options = ["AI / Machine Learning", "CUDA Development"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What will you use this machine for?")
        .items(&options)
        .default(0)
        .interact()
        .context("could not read the selected usage profile")?;

    Ok(match selection {
        0 => UsageProfile::AiMachineLearning,
        _ => UsageProfile::CudaDevelopment,
    })
}

pub fn confirm_install() -> Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue with this installation plan?")
        .default(false)
        .interact()
        .context("could not read installation confirmation")
}

pub fn confirm_uninstall() -> Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue with this uninstall plan?")
        .default(false)
        .interact()
        .context("could not read uninstall confirmation")
}
