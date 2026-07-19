use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{
    system::os,
    ui::{output, prompt},
};

const CUDA_TOOLKIT_PACKAGES: &[&str] = &[
    "*cuda*",
    "*cublas*",
    "*cufft*",
    "*cufile*",
    "*curand*",
    "*cusolver*",
    "*cusparse*",
    "*gds-tools*",
    "*npp*",
    "*nvjpeg*",
    "nsight*",
    "*nvvm*",
];

// Package families from NVIDIA's Ubuntu driver removal guide.
const NVIDIA_DRIVER_PACKAGES: &[&str] = &[
    "cuda-compat*",
    "cuda-drivers*",
    "libnvidia-cfg1*",
    "libnvidia-compute*",
    "libnvidia-decode*",
    "libnvidia-encode*",
    "libnvidia-extra*",
    "libnvidia-fbc1*",
    "libnvidia-gl*",
    "libnvidia-gpucomp*",
    "libnvidia-nscq*",
    "libnvsdm*",
    "libxnvctrl*",
    "nvidia-dkms*",
    "nvidia-driver*",
    "nvidia-fabricmanager*",
    "nvidia-firmware*",
    "nvidia-headless*",
    "nvidia-imex*",
    "nvidia-kernel*",
    "nvidia-modprobe*",
    "nvidia-open*",
    "nvidia-persistenced*",
    "nvidia-settings*",
    "nvidia-xconfig*",
    "xserver-xorg-video-nvidia*",
];

pub fn run() -> Result<()> {
    let os = os::detect()?;
    if !os.is_supported() {
        bail!(
            "cudaenv uninstall supports Ubuntu only (detected {}).",
            os.display_name()
        );
    }

    output::uninstall_plan(&uninstall_commands());

    if !prompt::confirm_uninstall()? {
        println!("\nUninstall cancelled. No changes were made.");
        return Ok(());
    }

    uninstall_cuda_toolkit()?;
    uninstall_driver()?;
    autoremove_packages()?;

    println!("\nCUDA Toolkit and NVIDIA driver packages were removed.");
    println!("Reboot Ubuntu before installing another driver.");
    Ok(())
}

fn uninstall_cuda_toolkit() -> Result<()> {
    run_apt(
        &["remove", "--purge", "--yes"],
        CUDA_TOOLKIT_PACKAGES,
        "remove CUDA Toolkit packages",
    )
}

fn uninstall_driver() -> Result<()> {
    run_apt(
        &["remove", "--autoremove", "--purge", "-V", "--yes"],
        NVIDIA_DRIVER_PACKAGES,
        "remove NVIDIA driver packages",
    )
}

fn autoremove_packages() -> Result<()> {
    run_apt(
        &["autoremove", "--purge", "--yes"],
        &[],
        "clean up unused CUDA and NVIDIA dependencies",
    )
}

fn run_apt(options: &[&str], packages: &[&str], action: &str) -> Result<()> {
    let status = Command::new("sudo")
        .arg("apt")
        .args(options)
        .args(packages)
        .status()
        .with_context(|| format!("could not start apt to {action}"))?;

    if !status.success() {
        bail!("apt failed to {action} (exit status: {status})");
    }

    Ok(())
}

fn uninstall_commands() -> Vec<String> {
    vec![
        display_apt_command(&["remove", "--purge", "--yes"], CUDA_TOOLKIT_PACKAGES),
        display_apt_command(
            &["remove", "--autoremove", "--purge", "-V", "--yes"],
            NVIDIA_DRIVER_PACKAGES,
        ),
        display_apt_command(&["autoremove", "--purge", "--yes"], &[]),
    ]
}

fn display_apt_command(options: &[&str], packages: &[&str]) -> String {
    let mut parts = vec!["sudo", "apt"];
    parts.extend_from_slice(options);

    let mut command = parts.join(" ");
    for package in packages {
        command.push_str(&format!(" \"{package}\""));
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_contains_toolkit_driver_and_cleanup_commands() {
        let commands = uninstall_commands();

        assert_eq!(commands.len(), 3);
        assert!(commands[0].contains("\"*cuda*\""));
        assert!(commands[1].contains("\"nvidia-driver*\""));
        assert_eq!(commands[2], "sudo apt autoremove --purge --yes");
    }
}
