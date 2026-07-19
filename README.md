# cudaenv

`cudaenv` inspects NVIDIA GPU environments and installs NVIDIA drivers from
official NVIDIA repositories. Driver installation supports Ubuntu, Debian,
RHEL, AlmaLinux, Rocky Linux, Oracle Linux, Fedora, Amazon Linux, Azure Linux,
openSUSE, SLES, and KylinOS. WSL is intentionally rejected because its NVIDIA
driver must be installed on the Windows host.

Repository targets are resolved from the exact distribution, release, and CPU
architecture. If NVIDIA does not publish that exact target, `cudaenv` stops
instead of borrowing another distribution's repository.

## Build and test

```bash
cargo build
cargo test
```

## Driver installation

```bash
cargo run -- install
cargo run -- install -- --driver open
cargo run -- install -- --driver proprietary --yes
cargo run -- install -- --driver auto --dry-run
```

`auto` selects open kernel modules for Turing and newer GPUs and proprietary
modules for Maxwell, Pascal, or Volta. A mixed system uses proprietary modules
if any GPU requires them. If PCI data cannot identify a GPU generation, the CLI
asks which flavor to install rather than guessing.

Every install prints the full repository and package command plan first. It asks
for confirmation unless `--yes` is supplied; `--dry-run` never changes the
system. This command installs only the NVIDIA driver and never the CUDA Toolkit.

Other inspection commands remain available:

```bash
cargo run -- status
cargo run -- doctor
cargo run -- uninstall
```
