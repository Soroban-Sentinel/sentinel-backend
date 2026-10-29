//! Mirror of the `.sentinel.toml` schema — parsed once and shared across crates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelConfig {
    pub version: String,
    pub contracts: Vec<ContractConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    pub name: String,
    pub path: String,
    pub invariants: Vec<String>,
    #[serde(default = "default_fuzz_timeout")]
    pub fuzz_timeout_secs: u64,
    #[serde(default)]
    pub fuzz_iterations: u64,
}

fn default_fuzz_timeout() -> u64 {
    60
}

impl SentinelConfig {
    /// Validate before using names in paths or launching any subprocess.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.version == "1", "unsupported config version");
        anyhow::ensure!(
            !self.contracts.is_empty() && self.contracts.len() <= 32,
            "expected 1..=32 contracts"
        );
        let mut names = std::collections::HashSet::new();
        for c in &self.contracts {
            anyhow::ensure!(
                !c.name.is_empty()
                    && c.name.len() <= 64
                    && c.name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
                "invalid contract name"
            );
            anyhow::ensure!(
                names.insert(c.name.replace('-', "_")),
                "duplicate normalized fuzz target"
            );
            let path = std::path::Path::new(&c.path);
            anyhow::ensure!(
                !c.path.is_empty()
                    && !path.is_absolute()
                    && path
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "contract path must be relative without traversal"
            );
            anyhow::ensure!(
                (1..=3600).contains(&c.fuzz_timeout_secs),
                "fuzz timeout must be 1..=3600 seconds"
            );
            anyhow::ensure!(
                !c.invariants.is_empty(),
                "at least one invariant is required"
            );
            let mut invariants = std::collections::HashSet::new();
            for invariant in &c.invariants {
                anyhow::ensure!(
                    matches!(
                        invariant.as_str(),
                        "balance_conservation" | "access_control" | "no_overflow"
                    ),
                    "unsupported invariant: {}",
                    invariant
                );
                anyhow::ensure!(invariants.insert(invariant), "duplicate invariant");
            }
        }
        Ok(())
    }

    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> SentinelConfig {
        SentinelConfig {
            version: "1".into(),
            contracts: vec![ContractConfig {
                name: "token".into(),
                path: "contracts/token".into(),
                invariants: vec!["no_overflow".into()],
                fuzz_timeout_secs: 60,
                fuzz_iterations: 0,
            }],
        }
    }
    #[test]
    fn accepts_valid_config() {
        assert!(config().validate().is_ok());
    }
    #[test]
    fn rejects_unsafe_names_and_paths() {
        for name in ["../escape", "", "a/b", "token\ncode"] {
            let mut c = config();
            c.contracts[0].name = name.into();
            assert!(c.validate().is_err());
        }
        for path in ["../escape", "/tmp/token", "contracts/../token", ""] {
            let mut c = config();
            c.contracts[0].path = path.into();
            assert!(c.validate().is_err());
        }
    }
    #[test]
    fn rejects_invalid_analysis_settings() {
        let mut c = config();
        c.version = "2".into();
        assert!(c.validate().is_err());
        let mut c = config();
        c.contracts.clear();
        assert!(c.validate().is_err());
        for timeout in [0, 3601, u64::MAX] {
            let mut c = config();
            c.contracts[0].fuzz_timeout_secs = timeout;
            assert!(c.validate().is_err());
        }
        let mut c = config();
        c.contracts[0].invariants = vec!["typo".into()];
        assert!(c.validate().is_err());
        let mut c = config();
        c.contracts.push(c.contracts[0].clone());
        assert!(c.validate().is_err());
        let mut c = config();
        c.contracts[0].name = "my-token".into();
        let mut second = c.contracts[0].clone();
        second.name = "my_token".into();
        c.contracts.push(second);
        assert!(c.validate().is_err());
    }
}
