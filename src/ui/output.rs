use crate::{system::os::OsInfo, ui::prompt::UsageProfile};

pub fn installation_plan(
    gpu: &str,
    profile: UsageProfile,
    driver_commands: &[String],
    toolkit_commands: Option<&[String]>,
) {
    println!("GPU:\n{gpu}");
    println!("\nProfile:\n{}", profile.label());
    println!("\nInstallation Plan\n");
    println!("✓ NVIDIA Driver");
    println!("  Package: Ubuntu recommended NVIDIA driver");
    print_commands(driver_commands);

    if let Some(commands) = toolkit_commands {
        println!("\n✓ CUDA Toolkit");
        println!("  Packages: cuda-keyring, cuda-toolkit");
        print_commands(commands);
    } else {
        println!("\n✗ CUDA Toolkit");
    }

    println!("\nNo system changes will be made until you confirm.");
}

fn print_commands(commands: &[String]) {
    for command in commands {
        println!("  $ {command}");
    }
}

pub fn system_status(os: &OsInfo, gpu: Option<&str>, driver: Option<&str>) {
    println!("GPU Environment");
    println!("\nOS:\n{}", os.display_name());
    println!("\nGPU:\n{}", gpu.unwrap_or("Not detected"));
    println!("\nDriver:\n{}", driver.unwrap_or("Not installed"));
}

pub fn diagnostics(gpu_detected: bool, driver_installed: bool, nvidia_smi: bool) {
    let healthy = gpu_detected && driver_installed && nvidia_smi;
    println!("NVIDIA Diagnostics\n");
    println!("{} NVIDIA GPU detected", mark(gpu_detected));
    println!("{} NVIDIA driver installed", mark(driver_installed));
    println!("{} nvidia-smi available", mark(nvidia_smi));

    if healthy {
        println!("\nHealthy");
        return;
    }

    println!("\nProblems found");
    if !gpu_detected {
        println!("- No NVIDIA GPU was detected by lspci or nvidia-smi.");
    }
    if !driver_installed {
        println!("- The NVIDIA driver does not appear to be installed or loaded.");
    }
    if !nvidia_smi {
        println!("- nvidia-smi is not available in PATH.");
    }
}

pub fn uninstall_plan(commands: &[String]) {
    println!("Uninstall Plan\n");
    println!("The following CUDA Toolkit and NVIDIA driver packages will be removed:");
    print_commands(commands);
    println!("\nThis operation changes system packages and cannot be automatically undone.");
}

fn mark(ok: bool) -> &'static str {
    if ok { "✓" } else { "✗" }
}
