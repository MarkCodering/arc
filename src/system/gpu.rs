use std::process::Command;

use anyhow::{Context, Result};

pub fn detect() -> Result<Option<String>> {
    if let Some(name) = detect_with_lspci()? {
        return Ok(Some(name));
    }

    detect_with_nvidia_smi()
}

fn detect_with_lspci() -> Result<Option<String>> {
    let output = match Command::new("lspci").arg("-nn").output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to run lspci"),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().find_map(parse_nvidia_lspci_line))
}

fn parse_nvidia_lspci_line(line: &str) -> Option<String> {
    let lowercase = line.to_lowercase();
    if !lowercase.contains("nvidia")
        || !(lowercase.contains("vga compatible controller") || lowercase.contains("3d controller"))
    {
        return None;
    }

    let description = line.split_once(": ").map_or(line, |(_, value)| value);
    let description = description
        .split(" (rev ")
        .next()
        .unwrap_or(description)
        .trim();

    Some(description.to_string())
}

fn detect_with_nvidia_smi() -> Result<Option<String>> {
    let output = match Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("failed to run nvidia-smi while detecting the GPU");
        }
    };

    if !output.status.success() {
        return Ok(None);
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_vga_controller() {
        let line = "01:00.0 VGA compatible controller: NVIDIA Corporation AD102 [GeForce RTX 4090] (rev a1)";
        assert_eq!(
            parse_nvidia_lspci_line(line).as_deref(),
            Some("NVIDIA Corporation AD102 [GeForce RTX 4090]")
        );
    }

    #[test]
    fn ignores_non_nvidia_controller() {
        let line = "00:02.0 VGA compatible controller: Intel Corporation Device 1234";
        assert!(parse_nvidia_lspci_line(line).is_none());
    }
}
