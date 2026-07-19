use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};

use crate::{
    model::{
        command::CommandSpec,
        device::GpuVendor,
        environment::{
            Confidence, DiagnosticCause, DiagnosticCheck, DiagnosticId, DiagnosticSection,
            DiagnosticStatus, Diagnostics, DriverInstallation, Fix, FixId, FixPlan,
        },
        system::OsInfo,
    },
    platform::{os, package_manager},
};

use super::{
    compatibility::{self, Compatibility},
    driver,
    gpu::{self, NvidiaGpu},
    policy, recipe, repository, state,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorProfile {
    ModelTraining,
    CudaDevelopment,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandEvidence {
    pub exists: bool,
    pub succeeded: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CudaSymlinkState {
    Missing,
    Valid(PathBuf),
    Broken(PathBuf),
    NotSymlink,
    Unavailable(String),
}

#[derive(Clone, Debug)]
pub struct NvidiaEvidence {
    pub os: OsInfo,
    pub gpus: Vec<NvidiaGpu>,
    pub driver: DriverInstallation,
    pub nvidia_module_loaded: bool,
    pub nvidia_smi: CommandEvidence,
    pub kernel_release: String,
    pub matching_kernel_headers: bool,
    pub secure_boot_enabled: Option<bool>,
    pub dkms_status: Option<String>,
    pub driver_version: Option<String>,
    pub toolkit_package_installed: bool,
    pub nvcc: CommandEvidence,
    pub nvcc_version: Option<String>,
    pub cuda_symlink: CudaSymlinkState,
    pub installed_cuda_versions: Vec<String>,
}

impl NvidiaEvidence {
    fn toolkit_installed(&self) -> bool {
        self.toolkit_package_installed || !self.installed_cuda_versions.is_empty()
    }
}

pub fn collect_evidence() -> Result<NvidiaEvidence> {
    let os = os::detect()?;
    let kernel_release = command_stdout("uname", &["-r"])
        .context("could not determine the running kernel release")?;
    let status = state::inspect(&os)?;
    let nvidia_smi = command_evidence(
        "nvidia-smi",
        &["--query-gpu=driver_version", "--format=csv,noheader"],
    );
    let mut nvcc = command_evidence("nvcc", &["--version"]);
    if !nvcc.exists {
        nvcc = command_evidence("/usr/local/cuda/bin/nvcc", &["--version"]);
    }
    let installed_cuda_versions = installed_cuda_versions();
    Ok(NvidiaEvidence {
        gpus: gpu::detect()?,
        driver: status.driver,
        nvidia_module_loaded: Path::new("/sys/module/nvidia").exists(),
        matching_kernel_headers: Path::new("/lib/modules")
            .join(&kernel_release)
            .join("build")
            .exists(),
        secure_boot_enabled: driver::secure_boot_enabled(),
        dkms_status: command_optional_stdout("dkms", &["status"]),
        driver_version: status.driver_version,
        toolkit_package_installed: package_manager::is_installed(
            os.package_manager(),
            "cuda-toolkit",
        )?,
        nvcc_version: parse_nvcc_version(&nvcc.stdout).map(str::to_owned),
        nvcc,
        cuda_symlink: cuda_symlink_state(Path::new("/usr/local/cuda")),
        installed_cuda_versions,
        kernel_release,
        nvidia_smi,
        os,
    })
}

pub fn detect(profile: DoctorProfile) -> Result<Diagnostics> {
    diagnose(collect_evidence()?, profile)
}

pub fn diagnose(e: NvidiaEvidence, profile: DoctorProfile) -> Result<Diagnostics> {
    let checks = checks(&e, profile);
    let fix_plan = fix_plan(&e, &checks, profile)?;
    Ok(Diagnostics {
        vendor: GpuVendor::Nvidia,
        checks,
        fix_plan,
    })
}

pub fn checks(e: &NvidiaEvidence, profile: DoctorProfile) -> Vec<DiagnosticCheck> {
    let gpu_ok = !e.gpus.is_empty();
    let os_resolution = repository::resolve(&e.os)
        .and_then(|_| recipe::validate_release(&e.os))
        .and_then(|_| e.os.ensure_driver_installable("NVIDIA"))
        .and_then(|_| {
            policy::resolve(
                &e.os,
                &e.gpus,
                super::driver::DriverPreference::Auto,
                e.nvcc_version.as_deref(),
                profile == DoctorProfile::CudaDevelopment,
            )
            .map(|_| ())
        });
    let mut result = vec![check(
        DiagnosticId::NvidiaGpu,
        DiagnosticSection::Hardware,
        "NVIDIA GPU detected",
        if gpu_ok {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Error
        },
        vec![if gpu_ok {
            format!("{} NVIDIA GPU(s) detected", e.gpus.len())
        } else {
            "No NVIDIA PCI device found".into()
        }],
        (!gpu_ok).then(|| "No NVIDIA GPU was detected by lspci or sysfs.".into()),
        vec![],
        vec![FixId::InspectHardware],
    )];
    result.push(check(
        DiagnosticId::OperatingSystem,
        DiagnosticSection::OperatingSystem,
        "Supported OS/GPU policy",
        if os_resolution.is_ok() {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Error
        },
        vec![format!("{} ({})", e.os.display_name(), e.os.architecture)],
        os_resolution.err().map(|error| error.to_string()),
        vec![DiagnosticId::NvidiaGpu],
        vec![],
    ));
    result.push(check(DiagnosticId::KernelHeaders, DiagnosticSection::OperatingSystem, "Headers for the running kernel", if e.matching_kernel_headers { DiagnosticStatus::Pass } else { DiagnosticStatus::Warning }, vec![format!("kernel {}: headers {}", e.kernel_release, yes_no(e.matching_kernel_headers))], (!e.matching_kernel_headers).then(|| "Matching kernel development packages must be installed before DKMS builds the driver.".into()), vec![], vec![FixId::InstallKernelHeaders]));
    result.push(check(
        DiagnosticId::SecureBoot,
        DiagnosticSection::OperatingSystem,
        "Secure Boot state",
        match e.secure_boot_enabled {
            Some(false) => DiagnosticStatus::Pass,
            _ => DiagnosticStatus::Warning,
        },
        vec![format!(
            "Secure Boot: {}",
            match e.secure_boot_enabled {
                Some(true) => "enabled",
                Some(false) => "disabled",
                None => "unknown",
            }
        )],
        e.secure_boot_enabled
            .is_none()
            .then(|| "Secure Boot state could not be determined.".into()),
        vec![],
        vec![],
    ));
    let (driver_status, driver_problem) = match &e.driver {
        DriverInstallation::Managed { .. } => (DiagnosticStatus::Pass, None),
        DriverInstallation::Missing => (DiagnosticStatus::Error, Some("No managed NVIDIA driver package installation was detected.".into())),
        DriverInstallation::BrokenManaged { .. } => (DiagnosticStatus::Error, Some("NVIDIA packages are installed, but the driver runtime is broken.".into())),
        DriverInstallation::Unmanaged { working: true, .. } => (DiagnosticStatus::Warning, Some("A working unmanaged driver is present; cudaenv will not overwrite it with repository packages.".into())),
        DriverInstallation::Unmanaged { working: false, .. } => (DiagnosticStatus::Error, Some("An unmanaged driver installation appears broken and must be removed with its original installer.".into())),
    };
    result.push(check(
        DiagnosticId::DriverPackage,
        DiagnosticSection::Driver,
        "NVIDIA driver installation method",
        driver_status,
        vec![e.driver.description()],
        driver_problem,
        vec![DiagnosticId::NvidiaGpu],
        vec![FixId::InstallDriver],
    ));
    push_dependent(
        &mut result,
        check(
            DiagnosticId::DriverModule,
            DiagnosticSection::Driver,
            "NVIDIA kernel module loaded",
            if e.nvidia_module_loaded {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Error
            },
            vec![format!(
                "/sys/module/nvidia: {}; DKMS: {}",
                if e.nvidia_module_loaded {
                    "present"
                } else {
                    "missing"
                },
                e.dkms_status.as_deref().unwrap_or("unavailable")
            )],
            (!e.nvidia_module_loaded).then(|| "The NVIDIA kernel module is not loaded.".into()),
            vec![DiagnosticId::DriverPackage],
            vec![FixId::RebuildDkms, FixId::Reboot],
        ),
    );
    push_dependent(
        &mut result,
        check(
            DiagnosticId::NvidiaSmi,
            DiagnosticSection::Driver,
            "nvidia-smi operational",
            if e.nvidia_smi.exists && e.nvidia_smi.succeeded {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Error
            },
            command_evidence_lines("nvidia-smi", &e.nvidia_smi),
            (!e.nvidia_smi.succeeded)
                .then(|| "nvidia-smi cannot communicate with the driver.".into()),
            vec![DiagnosticId::DriverModule],
            vec![FixId::DebugDriver],
        ),
    );
    let mismatch = version_mismatch(e);
    push_dependent(
        &mut result,
        check(
            DiagnosticId::DriverLibrary,
            DiagnosticSection::Driver,
            "Driver and userspace libraries match",
            if mismatch {
                DiagnosticStatus::Error
            } else {
                DiagnosticStatus::Pass
            },
            e.driver_version
                .as_ref()
                .map(|v| format!("driver version: {v}"))
                .into_iter()
                .collect(),
            mismatch.then(|| "NVML reports a driver/library version mismatch.".into()),
            vec![DiagnosticId::DriverPackage],
            vec![FixId::ReinstallDriverLibraries, FixId::Reboot],
        ),
    );

    let toolkit_present = e.toolkit_installed();
    let missing_status = if profile == DoctorProfile::CudaDevelopment {
        DiagnosticStatus::Error
    } else {
        DiagnosticStatus::Warning
    };
    result.push(check(
        DiagnosticId::ToolkitInstall,
        DiagnosticSection::CudaToolkit,
        "CUDA Toolkit installation",
        if toolkit_present {
            DiagnosticStatus::Pass
        } else {
            missing_status
        },
        vec![format!(
            "cuda-toolkit package: {}; installed versions: {}",
            yes_no(e.toolkit_package_installed),
            list_or_none(&e.installed_cuda_versions)
        )],
        (!toolkit_present).then(|| {
            if profile == DoctorProfile::CudaDevelopment {
                "CUDA development requires a Toolkit.".into()
            } else {
                "No Toolkit detected; this is normal for frameworks that bundle a CUDA runtime."
                    .into()
            }
        }),
        vec![],
        if profile == DoctorProfile::CudaDevelopment {
            vec![FixId::InstallToolkit]
        } else {
            vec![]
        },
    ));
    push_dependent(
        &mut result,
        check(
            DiagnosticId::Nvcc,
            DiagnosticSection::CudaToolkit,
            "nvcc available",
            if e.nvcc.exists && e.nvcc.succeeded && e.nvcc_version.is_some() {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Error
            },
            command_evidence_lines("nvcc", &e.nvcc),
            (!(e.nvcc.exists && e.nvcc.succeeded && e.nvcc_version.is_some()))
                .then(|| "A Toolkit is present, but nvcc is missing or broken.".into()),
            vec![DiagnosticId::ToolkitInstall],
            vec![FixId::InstallToolkit, FixId::DebugToolkit],
        ),
    );
    push_dependent(
        &mut result,
        check(
            DiagnosticId::CudaSymlink,
            DiagnosticSection::CudaToolkit,
            "/usr/local/cuda configuration",
            match e.cuda_symlink {
                CudaSymlinkState::Valid(_) => DiagnosticStatus::Pass,
                CudaSymlinkState::Missing if e.installed_cuda_versions.len() <= 1 => {
                    DiagnosticStatus::Warning
                }
                _ => DiagnosticStatus::Error,
            },
            vec![cuda_symlink_description(&e.cuda_symlink)],
            (!matches!(e.cuda_symlink, CudaSymlinkState::Valid(_)))
                .then(|| "/usr/local/cuda does not point to a valid Toolkit.".into()),
            vec![DiagnosticId::ToolkitInstall],
            vec![FixId::RepairCudaSymlink],
        ),
    );
    let compatibility = e
        .driver_version
        .as_deref()
        .zip(e.nvcc_version.as_deref())
        .and_then(|(driver, toolkit)| compatibility::evaluate(driver, toolkit));
    push_dependent(&mut result, check(DiagnosticId::DriverToolkitCompatibility, DiagnosticSection::CudaToolkit, "Driver supports CUDA Toolkit", match compatibility { Some(Compatibility::Incompatible) => DiagnosticStatus::Error, Some(Compatibility::MinorVersionCompatible) => DiagnosticStatus::Warning, _ => DiagnosticStatus::Pass }, vec![format!("driver: {}; toolkit: {}; compatibility: {:?}", e.driver_version.as_deref().unwrap_or("unknown"), e.nvcc_version.as_deref().unwrap_or("unknown"), compatibility)], (compatibility == Some(Compatibility::Incompatible)).then(|| "The complete driver version is below the Toolkit's minimum compatibility version.".into()), vec![DiagnosticId::NvidiaSmi, DiagnosticId::Nvcc], vec![FixId::UpgradeDriver, FixId::Reboot]));
    result
}

fn push_dependent(result: &mut Vec<DiagnosticCheck>, mut check: DiagnosticCheck) {
    if check.dependencies.iter().any(|id| {
        result
            .iter()
            .any(|prior| prior.id == *id && prior.status != DiagnosticStatus::Pass)
    }) {
        check.status = DiagnosticStatus::Skipped;
        check.problem = Some("Skipped because a prerequisite check did not pass.".into());
        check.recommended_fixes.clear();
    }
    result.push(check);
}
#[allow(clippy::too_many_arguments)]
fn check(
    id: DiagnosticId,
    section: DiagnosticSection,
    name: &str,
    status: DiagnosticStatus,
    evidence: Vec<String>,
    problem: Option<String>,
    dependencies: Vec<DiagnosticId>,
    fixes: Vec<FixId>,
) -> DiagnosticCheck {
    DiagnosticCheck {
        id,
        section,
        name: name.into(),
        status,
        evidence,
        problem,
        dependencies,
        recommended_fixes: fixes,
    }
}

pub fn fix_plan(
    e: &NvidiaEvidence,
    checks: &[DiagnosticCheck],
    profile: DoctorProfile,
) -> Result<FixPlan> {
    let failed = |id| {
        checks
            .iter()
            .any(|c| c.id == id && c.status == DiagnosticStatus::Error)
    };
    let mut causes = Vec::new();
    if failed(DiagnosticId::NvidiaGpu) {
        causes.push(cause(
            "The NVIDIA GPU is not visible",
            vec![FixId::InspectHardware],
        ));
    }
    if failed(DiagnosticId::OperatingSystem) {
        causes.push(cause("The OS/GPU combination is unsupported", vec![]));
    }
    if failed(DiagnosticId::DriverPackage) {
        causes.push(cause(
            "The NVIDIA driver installation is missing or broken",
            vec![FixId::InstallDriver],
        ));
    }
    if e.driver.is_managed() && !e.nvidia_module_loaded && !e.matching_kernel_headers {
        causes.push(cause(
            "Headers for the running kernel are missing",
            vec![
                FixId::InstallKernelHeaders,
                FixId::RebuildDkms,
                FixId::Reboot,
            ],
        ));
    }
    if version_mismatch(e) {
        causes.push(cause(
            "NVIDIA driver and userspace libraries do not match",
            vec![FixId::ReinstallDriverLibraries, FixId::Reboot],
        ));
    }
    if e.toolkit_installed() && failed(DiagnosticId::Nvcc) {
        causes.push(cause(
            "The CUDA Toolkit is partially installed or broken",
            vec![FixId::InstallToolkit, FixId::DebugToolkit],
        ));
    }
    if profile == DoctorProfile::CudaDevelopment && !e.toolkit_installed() {
        causes.push(cause(
            "CUDA development was requested but the Toolkit is missing",
            vec![FixId::InstallToolkit],
        ));
    }
    if failed(DiagnosticId::DriverToolkitCompatibility) {
        causes.push(cause(
            "The NVIDIA driver is incompatible with the CUDA Toolkit",
            vec![FixId::UpgradeDriver, FixId::Reboot],
        ));
    }
    Ok(FixPlan::new(causes, available_fixes(e)?))
}
fn cause(title: &str, fixes: Vec<FixId>) -> DiagnosticCause {
    DiagnosticCause {
        title: title.into(),
        confidence: Confidence::High,
        evidence: vec![],
        fixes,
    }
}
fn available_fixes(e: &NvidiaEvidence) -> Result<Vec<Fix>> {
    let prerequisites = recipe::prerequisites(&e.os, &e.kernel_release).unwrap_or_default();
    Ok(vec![
        fix(
            FixId::InspectHardware,
            "Verify that the NVIDIA GPU is visible",
            vec![CommandSpec::new("lspci", ["-nnk", "-d", "10de:"])],
            5,
        ),
        fix(
            FixId::InstallKernelHeaders,
            "Install exact prerequisites for the running kernel",
            prerequisites,
            10,
        ),
        fix(
            FixId::InstallDriver,
            "Review the normal cudaenv installation plan",
            vec![CommandSpec::new(
                "cudaenv",
                ["install", "--profile", "model-training", "--dry-run"],
            )],
            20,
        ),
        fix(
            FixId::UpgradeDriver,
            "Upgrade the managed NVIDIA driver",
            vec![CommandSpec::new(
                "cudaenv",
                ["install", "--profile", "model-training", "--dry-run"],
            )],
            20,
        ),
        fix(
            FixId::ReinstallDriverLibraries,
            "Reinstall the managed NVIDIA packages",
            vec![CommandSpec::new(
                "cudaenv",
                ["install", "--profile", "model-training", "--dry-run"],
            )],
            30,
        ),
        fix(
            FixId::RebuildDkms,
            "Rebuild NVIDIA DKMS modules",
            vec![CommandSpec::sudo(
                "dkms",
                ["autoinstall", "-k", &e.kernel_release],
            )],
            40,
        ),
        fix(
            FixId::InstallToolkit,
            "Install or repair the CUDA Toolkit",
            vec![CommandSpec::new(
                "cudaenv",
                ["install", "--profile", "cuda-development", "--dry-run"],
            )],
            50,
        ),
        fix(
            FixId::RepairCudaSymlink,
            "Repair /usr/local/cuda",
            vec![],
            60,
        ),
        fix(
            FixId::DebugDriver,
            "Collect driver evidence",
            vec![
                CommandSpec::new("journalctl", ["-k", "-b", "-g", "NVRM|nvidia|nouveau"]),
                CommandSpec::new("dkms", ["status"]),
            ],
            80,
        ),
        fix(
            FixId::DebugToolkit,
            "Inspect Toolkit paths",
            vec![
                CommandSpec::new("readlink", ["-f", "/usr/local/cuda"]),
                CommandSpec::new("nvcc", ["--version"]),
            ],
            80,
        ),
        fix(
            FixId::Reboot,
            "Reboot to load the repaired driver",
            vec![CommandSpec::sudo("systemctl", ["reboot"])],
            90,
        ),
    ])
}
fn fix(id: FixId, title: &str, commands: Vec<CommandSpec>, order: u16) -> Fix {
    Fix {
        id,
        title: title.into(),
        commands,
        manual_steps: vec![],
        order,
    }
}

fn command_evidence(program: &str, args: &[&str]) -> CommandEvidence {
    match Command::new(program).args(args).output() {
        Ok(output) => CommandEvidence {
            exists: true,
            succeeded: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().into(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().into(),
        },
        Err(error) => CommandEvidence {
            stderr: error.to_string(),
            ..Default::default()
        },
    }
}
fn command_stdout(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output()?;
    anyhow::ensure!(output.status.success(), "{program} failed");
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
fn command_optional_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().into())
}
fn installed_cuda_versions() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/usr/local") else {
        return vec![];
    };
    let mut values = entries
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_prefix("cuda-"))
                .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
                .map(Into::into)
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
fn cuda_symlink_state(path: &Path) -> CudaSymlinkState {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.file_type().is_symlink() => CudaSymlinkState::NotSymlink,
        Ok(_) => match fs::read_link(path) {
            Ok(target) => {
                let resolved = if target.is_absolute() {
                    target.clone()
                } else {
                    path.parent().unwrap_or(Path::new("/")).join(&target)
                };
                if resolved.exists() {
                    CudaSymlinkState::Valid(target)
                } else {
                    CudaSymlinkState::Broken(target)
                }
            }
            Err(e) => CudaSymlinkState::Unavailable(e.to_string()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => CudaSymlinkState::Missing,
        Err(e) => CudaSymlinkState::Unavailable(e.to_string()),
    }
}
fn parse_nvcc_version(output: &str) -> Option<&str> {
    let (_, rest) = output.split_once("release ")?;
    rest.split(|c: char| c == ',' || c.is_whitespace())
        .find(|p| !p.is_empty())
}
fn version_mismatch(e: &NvidiaEvidence) -> bool {
    format!("{}\n{}", e.nvidia_smi.stdout, e.nvidia_smi.stderr)
        .to_ascii_lowercase()
        .contains("driver/library version mismatch")
}
fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join(", ")
    }
}
fn command_evidence_lines(name: &str, value: &CommandEvidence) -> Vec<String> {
    vec![
        format!(
            "{name}: {}",
            if !value.exists {
                "not found"
            } else if value.succeeded {
                "succeeded"
            } else {
                "failed"
            }
        ),
        format!("stderr: {}", value.stderr),
    ]
}
fn cuda_symlink_description(state: &CudaSymlinkState) -> String {
    match state {
        CudaSymlinkState::Missing => "/usr/local/cuda: missing".into(),
        CudaSymlinkState::Valid(p) => format!("/usr/local/cuda -> {} (valid)", p.display()),
        CudaSymlinkState::Broken(p) => format!("/usr/local/cuda -> {} (broken)", p.display()),
        CudaSymlinkState::NotSymlink => "/usr/local/cuda is not a symlink".into(),
        CudaSymlinkState::Unavailable(e) => format!("/usr/local/cuda unavailable: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::gpu::Generation;
    use super::*;
    use crate::model::{
        environment::{DriverFlavorState, DriverPackageScope},
        system::Distribution,
    };
    fn evidence() -> NvidiaEvidence {
        NvidiaEvidence {
            os: OsInfo {
                distribution: Distribution::Ubuntu,
                name: "Ubuntu".into(),
                version_id: "24.04".into(),
                architecture: "x86_64".into(),
                is_wsl: false,
            },
            gpus: vec![NvidiaGpu {
                name: "GPU".into(),
                pci_device_id: None,
                generation: Generation::TuringOrNewer,
            }],
            driver: DriverInstallation::Managed {
                flavor: DriverFlavorState::Open,
                scope: DriverPackageScope::ComputeOnly,
                branch: None,
                packages: vec![],
            },
            nvidia_module_loaded: true,
            nvidia_smi: CommandEvidence {
                exists: true,
                succeeded: true,
                stdout: "570.26".into(),
                stderr: "".into(),
            },
            kernel_release: "6.8.0-generic".into(),
            matching_kernel_headers: true,
            secure_boot_enabled: Some(false),
            dkms_status: None,
            driver_version: Some("570.26".into()),
            toolkit_package_installed: false,
            nvcc: CommandEvidence::default(),
            nvcc_version: None,
            cuda_symlink: CudaSymlinkState::Missing,
            installed_cuda_versions: vec![],
        }
    }
    #[test]
    fn missing_toolkit_is_profile_aware() {
        let e = evidence();
        assert!(
            !diagnose(e.clone(), DoctorProfile::ModelTraining)
                .unwrap()
                .has_errors()
        );
        assert!(
            diagnose(e, DoctorProfile::CudaDevelopment)
                .unwrap()
                .has_errors()
        );
    }
    #[test]
    fn compatibility_warning_is_not_an_error() {
        let mut e = evidence();
        e.toolkit_package_installed = true;
        e.installed_cuda_versions = vec!["12.8".into()];
        e.nvcc = CommandEvidence {
            exists: true,
            succeeded: true,
            stdout: "release 12.8,".into(),
            stderr: "".into(),
        };
        e.nvcc_version = Some("12.8".into());
        e.driver_version = Some("525.60.13".into());
        assert_eq!(
            checks(&e, DoctorProfile::CudaDevelopment)
                .iter()
                .find(|c| c.id == DiagnosticId::DriverToolkitCompatibility)
                .unwrap()
                .status,
            DiagnosticStatus::Warning
        );
    }
}
