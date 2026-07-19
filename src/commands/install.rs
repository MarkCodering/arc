use anyhow::{Result, bail};

use crate::{
    cli::InstallArgs,
    system::{
        command::CommandSpec,
        driver::{self, DriverFlavor, Selection},
        gpu, os, repository,
    },
    ui::prompt,
};

pub fn run(args: InstallArgs) -> Result<()> {
    let os = os::detect()?;
    os.ensure_driver_installable()?;
    let gpus = gpu::detect()?;
    if gpus.is_empty() {
        bail!(
            "No NVIDIA GPU was detected. Check that the GPU is visible and try again."
        );
    }

    let flavor = match driver::select(args.driver, &gpus) {
        Selection::Selected(flavor) => flavor,
        Selection::NeedsUserChoice => prompt::select_driver_flavor()?,
    };
    if os.distribution == os::Distribution::AzureLinux && flavor == DriverFlavor::Proprietary {
        bail!("Azure Linux supports only NVIDIA open kernel modules; use --driver open.");
    }

    let repository = repository::resolve(&os)?;
    let mut commands = repository::repository_commands(&os, &repository);
    commands.push(install_command(os.package_manager(), flavor));
    print_plan(&os, &gpus, flavor, &repository, &commands);

    if args.dry_run {
        println!("\nDry run complete. No changes were made.");
        return Ok(());
    }
    if !args.yes && !prompt::confirm_install()? {
        println!("\nInstallation cancelled. No changes were made.");
        return Ok(());
    }
    for command in &commands {
        command.execute()?;
    }
    println!("\nNVIDIA driver installation completed. Reboot to load the driver.");
    Ok(())
}

fn install_command(manager: os::PackageManager, flavor: DriverFlavor) -> CommandSpec {
    let package = flavor.package();
    match manager {
        os::PackageManager::AptGet => CommandSpec::sudo("apt-get", ["install", "-y", package]),
        os::PackageManager::Dnf => CommandSpec::sudo("dnf", ["install", "-y", package]),
        os::PackageManager::Tdnf => CommandSpec::sudo("tdnf", ["install", "-y", package]),
        os::PackageManager::Zypper => {
            CommandSpec::sudo("zypper", ["--non-interactive", "install", package])
        }
    }
}

fn print_plan(
    os: &os::OsInfo,
    gpus: &[gpu::Gpu],
    flavor: DriverFlavor,
    repository: &repository::Repository,
    commands: &[CommandSpec],
) {
    println!("NVIDIA Driver Installation Plan\n");
    println!("OS: {}", os.display_name());
    println!("Package manager: {:?}", os.package_manager());
    println!("Repository: {}", repository.base_url);
    println!("Driver: {}", flavor.package());
    println!("GPU(s):");
    for gpu in gpus {
        println!("  - {} ({:?})", gpu.name, gpu.generation);
    }
    println!("\nCommands:");
    for command in commands {
        println!("  $ {}", command.display());
    }
    println!("\nThe CUDA Toolkit will not be installed.");
    println!("No system changes will be made until you confirm.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_install_commands_for_each_manager() {
        assert_eq!(
            install_command(os::PackageManager::AptGet, DriverFlavor::Open).display(),
            "sudo apt-get install -y nvidia-open"
        );
        assert_eq!(
            install_command(os::PackageManager::Dnf, DriverFlavor::Proprietary).display(),
            "sudo dnf install -y cuda-drivers"
        );
        assert_eq!(
            install_command(os::PackageManager::Tdnf, DriverFlavor::Open).display(),
            "sudo tdnf install -y nvidia-open"
        );
        assert_eq!(
            install_command(os::PackageManager::Zypper, DriverFlavor::Proprietary).display(),
            "sudo zypper --non-interactive install cuda-drivers"
        );
    }
}
