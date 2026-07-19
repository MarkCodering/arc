use anyhow::Result;

#[derive(Debug, PartialEq, Eq)]
pub struct OsInfo {
    pub name: String,
    pub version_id: String,
}

impl OsInfo {
    pub fn is_supported(&self) -> bool {
        self.name.eq_ignore_ascii_case("ubuntu")
    }

    pub fn display_name(&self) -> String {
        format!("{} {}", self.name, self.version_id)
    }
}

pub fn detect() -> Result<OsInfo> {
    let info = os_info::get();
    let name = info.os_type().to_string();

    Ok(OsInfo {
        name,
        version_id: info.version().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_all_ubuntu_releases() {
        for version in ["18.04", "20.04", "22.04", "24.04", "25.10", "26.04"] {
            let info = OsInfo {
                name: "Ubuntu".to_string(),
                version_id: version.to_string(),
            };

            assert!(info.is_supported(), "Ubuntu {version} should be supported");
        }
    }

    #[test]
    fn rejects_non_ubuntu_distribution() {
        let info = OsInfo {
            name: "Debian".to_string(),
            version_id: "12".to_string(),
        };

        assert!(!info.is_supported());
    }
}
