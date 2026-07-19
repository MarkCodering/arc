# cudaenv

`cudaenv` is an MVP command-line tool that inspects an Ubuntu NVIDIA GPU
environment, previews driver installation, and removes CUDA/NVIDIA packages
after explicit confirmation.

Supported operating systems: all Ubuntu releases, including interim and
end-of-life releases. Driver installation and package removal use Ubuntu's
native tools.

CUDA Toolkit installation additionally requires NVIDIA to publish a repository
for the detected Ubuntu release. If no matching repository exists, `cudaenv`
reports that the toolkit is unavailable instead of substituting packages from a
different Ubuntu release.

## Build

Install a current stable Rust toolchain, then run:

```bash
cargo build
```

The binary is written to `target/debug/cudaenv`. Run the tests with:

```bash
cargo test
```

## Commands

```bash
cargo run -- install
cargo run -- status
cargo run -- doctor
cargo run -- uninstall
```

`install` asks for a usage profile and prints a plan. For CUDA Development, it requests
NVIDIA's live Ubuntu repository index and includes the complete repository and
toolkit commands. It then asks for confirmation before installing the recommended
driver and selected toolkit packages. `uninstall` shows the exact APT commands,
asks for confirmation, and then removes the CUDA Toolkit and NVIDIA driver package
families documented by NVIDIA.

> **Warning:** `cudaenv uninstall` removes system-wide CUDA Toolkit and NVIDIA
> driver packages, including packages that may not have been installed by
> `cudaenv`. The confirmation defaults to No.

## Example output

```text
$ cudaenv status
GPU Environment

OS:
Ubuntu 24.04

GPU:
NVIDIA Corporation AD102 [GeForce RTX 4090]

Driver:
570.86
```

```text
$ cudaenv doctor
NVIDIA Diagnostics

✓ NVIDIA GPU detected
✓ NVIDIA driver installed
✓ nvidia-smi available

Healthy
```

```text
$ cudaenv install
? What will you use this machine for? AI / Machine Learning
GPU:
NVIDIA Corporation AD102 [GeForce RTX 4090]

Profile:
AI / Machine Learning

Installation Plan

✓ NVIDIA Driver
  Package: Ubuntu recommended NVIDIA driver
  $ sudo ubuntu-drivers install

✗ CUDA Toolkit

No system changes will be made until you confirm.
? Continue with this installation plan? No

Installation cancelled. No changes were made.
```
