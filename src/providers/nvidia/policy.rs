use anyhow::{Result, bail};

use crate::model::system::{Distribution, OsInfo};

use super::{
    driver::{DriverFlavor, DriverPreference},
    gpu::{Generation, NvidiaGpu},
};

pub const LEGACY_DRIVER_BRANCH: u32 = 580;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverPolicy {
    pub flavor: DriverFlavor,
    pub branch: Option<u32>,
    pub legacy_gpu: bool,
}

pub fn resolve(
    os: &OsInfo,
    gpus: &[NvidiaGpu],
    preference: DriverPreference,
    toolkit_version: Option<&str>,
    cuda_development: bool,
) -> Result<DriverPolicy> {
    if gpus.is_empty() {
        bail!("No NVIDIA GPU was detected. Check that the GPU is visible and try again.");
    }
    let legacy = gpus
        .iter()
        .any(|gpu| gpu.generation == Generation::MaxwellPascalVolta);
    let unknown = gpus.iter().any(|gpu| gpu.generation == Generation::Unknown);
    if legacy && preference == DriverPreference::Open {
        bail!(
            "--driver open is incompatible with Maxwell, Pascal, and Volta GPUs; these GPUs require the proprietary R580 branch."
        );
    }
    if legacy && os.distribution == Distribution::AzureLinux {
        bail!(
            "Maxwell, Pascal, and Volta GPUs are unsupported on Azure Linux because Azure Linux supports only open NVIDIA kernel modules."
        );
    }
    if unknown && preference == DriverPreference::Auto {
        bail!(
            "Could not determine whether every NVIDIA GPU supports open kernel modules. Re-run with --driver open or --driver proprietary after checking every GPU generation."
        );
    }
    let flavor = if legacy {
        DriverFlavor::Proprietary
    } else {
        match preference {
            DriverPreference::Auto | DriverPreference::Open => DriverFlavor::Open,
            DriverPreference::Proprietary => DriverFlavor::Proprietary,
        }
    };
    if os.distribution == Distribution::AzureLinux && flavor == DriverFlavor::Proprietary {
        bail!(
            "Azure Linux supports only NVIDIA open kernel modules; proprietary modules cannot be selected."
        );
    }
    if legacy && cuda_development {
        let Some(version) = toolkit_version else {
            bail!(
                "CUDA development on Maxwell, Pascal, and Volta requires an explicit supported CUDA 12.x Toolkit, for example --toolkit 12.8."
            );
        };
        if toolkit_major(version) != Some(12) {
            bail!(
                "Maxwell, Pascal, and Volta GPUs do not support CUDA 13.x; select a supported CUDA 12.x Toolkit."
            );
        }
    }
    Ok(DriverPolicy {
        flavor,
        branch: legacy.then_some(LEGACY_DRIVER_BRANCH),
        legacy_gpu: legacy,
    })
}

fn toolkit_major(version: &str) -> Option<u32> {
    version.split(['.', '-']).next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(generation: Generation) -> NvidiaGpu {
        NvidiaGpu {
            name: "GPU".into(),
            pci_device_id: None,
            generation,
        }
    }
    fn os(distribution: Distribution) -> OsInfo {
        OsInfo {
            distribution,
            name: "Test".into(),
            version_id: "3.0".into(),
            architecture: "x86_64".into(),
            is_wsl: false,
        }
    }

    #[test]
    fn generation_policy_is_safe_for_modern_legacy_unknown_and_mixed_hosts() {
        assert_eq!(
            resolve(
                &os(Distribution::Ubuntu),
                &[gpu(Generation::TuringOrNewer)],
                DriverPreference::Auto,
                None,
                false
            )
            .unwrap()
            .flavor,
            DriverFlavor::Open
        );
        let legacy = resolve(
            &os(Distribution::Ubuntu),
            &[gpu(Generation::MaxwellPascalVolta)],
            DriverPreference::Auto,
            Some("12.8"),
            true,
        )
        .unwrap();
        assert_eq!(
            (legacy.flavor, legacy.branch),
            (DriverFlavor::Proprietary, Some(580))
        );
        assert!(
            resolve(
                &os(Distribution::Ubuntu),
                &[gpu(Generation::MaxwellPascalVolta)],
                DriverPreference::Open,
                None,
                false
            )
            .is_err()
        );
        assert!(
            resolve(
                &os(Distribution::Ubuntu),
                &[gpu(Generation::Unknown)],
                DriverPreference::Auto,
                None,
                false
            )
            .is_err()
        );
        assert!(
            resolve(
                &os(Distribution::Ubuntu),
                &[
                    gpu(Generation::TuringOrNewer),
                    gpu(Generation::MaxwellPascalVolta)
                ],
                DriverPreference::Auto,
                Some("13.0"),
                true
            )
            .is_err()
        );
        assert!(
            resolve(
                &os(Distribution::AzureLinux),
                &[gpu(Generation::MaxwellPascalVolta)],
                DriverPreference::Proprietary,
                Some("12.8"),
                true
            )
            .is_err()
        );
    }
}
