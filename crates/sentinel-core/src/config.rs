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
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}
