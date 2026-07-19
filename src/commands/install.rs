use std::{process::Command, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::{StatusCode, blocking::Client};

use crate::{
    system::{gpu, os},
    ui::{output, prompt},
};

pub fn run() -> Result<()> {
    let os = os::detect()?;
    if !os.is_supported() {
        bail!(
            "cudaenv install supports Ubuntu only (detected {}).",
            os.display_name()
        );
    }

    let gpu = gpu::detect()?;
    let Some(gpu) = gpu else {
        bail!("No NVIDIA GPU was detected. Check that the GPU is visible to Ubuntu and try again.");
    };

    let profile = prompt::select_usage_profile()?;
    let driver_commands = driver_install_commands();
    let toolkit_source = match profile {
        prompt::UsageProfile::AiMachineLearning => None,
        prompt::UsageProfile::CudaDevelopment => Some(resolve_cuda_toolkit(&os)?),
    };
    let toolkit_commands = toolkit_source.as_ref().map(cuda_toolkit_commands);

    output::installation_plan(&gpu, profile, &driver_commands, toolkit_commands.as_deref());

    if !prompt::confirm_install()? {
        println!("\nInstallation cancelled. No changes were made.");
        return Ok(());
    }

    install_driver()?;
    if let Some(source) = toolkit_source {
        install_cuda_toolkit(&source)?;
    }

    println!("\nInstallation completed successfully.");
    println!("Reboot Ubuntu to load the NVIDIA driver.");
    Ok(())
}

#[derive(Debug)]
struct CudaToolkitSource {
    keyring_url: String,
    keyring_path: String,
}

fn driver_install_commands() -> Vec<String> {
    vec!["sudo ubuntu-drivers install".to_string()]
}

fn install_driver() -> Result<()> {
    run_command(
        "sudo",
        &["ubuntu-drivers", "install"],
        "install the recommended NVIDIA driver",
    )
}

/// Request NVIDIA's live CUDA repository and resolve its current keyring package.
fn resolve_cuda_toolkit(os: &os::OsInfo) -> Result<CudaToolkitSource> {
    let repository = cuda_repository_url(os)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("cudaenv/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("could not create the NVIDIA repository client")?;

    let response = client
        .get(&repository)
        .send()
        .context("could not request the NVIDIA CUDA repository")?;

    if response.status() == StatusCode::NOT_FOUND {
        bail!(
            "NVIDIA does not publish a CUDA repository for Ubuntu {}; CUDA Toolkit installation is unavailable on this release",
            os.version_id
        );
    }

    let index = response
        .error_for_status()
        .context("NVIDIA's CUDA repository returned an error")?
        .text()
        .context("could not read the NVIDIA CUDA repository response")?;

    let keyring_package = find_cuda_keyring_package(&index).context(
        "NVIDIA's CUDA repository did not contain a cuda-keyring package; try again later",
    )?;
    let keyring_url = format!("{repository}{keyring_package}");
    let keyring_path = format!("/tmp/{keyring_package}");

    Ok(CudaToolkitSource {
        keyring_url,
        keyring_path,
    })
}

fn cuda_toolkit_commands(source: &CudaToolkitSource) -> Vec<String> {
    vec![
        format!(
            "curl --fail --location --output {} {}",
            source.keyring_path, source.keyring_url
        ),
        format!("sudo dpkg -i {}", source.keyring_path),
        "sudo apt-get update".to_string(),
        "sudo apt-get install -y cuda-toolkit".to_string(),
    ]
}

fn install_cuda_toolkit(source: &CudaToolkitSource) -> Result<()> {
    run_command(
        "curl",
        &[
            "--fail",
            "--location",
            "--output",
            &source.keyring_path,
            &source.keyring_url,
        ],
        "download NVIDIA's CUDA repository keyring",
    )?;
    run_command(
        "sudo",
        &["dpkg", "-i", &source.keyring_path],
        "install NVIDIA's CUDA repository keyring",
    )?;
    run_command("sudo", &["apt-get", "update"], "update APT metadata")?;
    run_command(
        "sudo",
        &["apt-get", "install", "--yes", "cuda-toolkit"],
        "install the CUDA Toolkit",
    )
}

fn run_command(program: &str, arguments: &[&str], action: &str) -> Result<()> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| format!("could not start {program} to {action}"))?;

    if !status.success() {
        bail!("failed to {action} (exit status: {status})");
    }

    Ok(())
}

fn cuda_repository_url(os: &os::OsInfo) -> Result<String> {
    let distribution = ubuntu_cuda_distribution(&os.version_id)?;
    let architecture = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "sbsa",
        architecture => bail!("CUDA Toolkit installation is not supported on {architecture}"),
    };

    Ok(format!(
        "https://developer.download.nvidia.com/compute/cuda/repos/{distribution}/{architecture}/"
    ))
}

fn ubuntu_cuda_distribution(version: &str) -> Result<String> {
    let Some((year, month)) = version.split_once('.') else {
        bail!("invalid Ubuntu version {version:?}; expected YY.MM (for example, 24.04)");
    };

    if year.len() != 2
        || month.len() != 2
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || !month.bytes().all(|byte| byte.is_ascii_digit())
    {
        bail!("invalid Ubuntu version {version:?}; expected YY.MM (for example, 24.04)");
    }

    Ok(format!("ubuntu{year}{month}"))
}

fn find_cuda_keyring_package(index: &str) -> Option<&str> {
    index
        .split(['\"', '\''])
        .filter(|value| value.starts_with("cuda-keyring_") && value.ends_with("_all.deb"))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_latest_keyring_package_in_repository_index() {
        let index = r#"
            <a href="cuda-keyring_1.0-1_all.deb">old</a>
            <a href="cuda-keyring_1.1-1_all.deb">current</a>
        "#;

        assert_eq!(
            find_cuda_keyring_package(index),
            Some("cuda-keyring_1.1-1_all.deb")
        );
    }

    #[test]
    fn driver_plan_uses_ubuntu_recommended_driver() {
        assert_eq!(driver_install_commands(), ["sudo ubuntu-drivers install"]);
    }

    #[test]
    fn derives_cuda_distributions_for_ubuntu_releases() {
        assert_eq!(ubuntu_cuda_distribution("20.04").unwrap(), "ubuntu2004");
        assert_eq!(ubuntu_cuda_distribution("25.10").unwrap(), "ubuntu2510");
        assert_eq!(ubuntu_cuda_distribution("26.04").unwrap(), "ubuntu2604");
    }

    #[test]
    fn rejects_malformed_ubuntu_versions() {
        for version in ["24", "24.4", "2024.04", "24.04.1", "rolling", ""] {
            assert!(ubuntu_cuda_distribution(version).is_err(), "{version:?}");
        }
    }

    #[test]
    fn builds_cuda_repository_url_for_current_architecture() {
        let os = os::OsInfo {
            name: "Ubuntu".to_string(),
            version_id: "20.04".to_string(),
        };

        let expected_architecture = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "sbsa",
            _ => return,
        };

        assert_eq!(
            cuda_repository_url(&os).unwrap(),
            format!(
                "https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2004/{expected_architecture}/"
            )
        );
    }
}
