//! Manages the seed corpus directory for each contract target.
//!
//! A seed corpus pre-populates the fuzzer with valid-looking inputs,
//! dramatically improving early coverage.

use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

pub struct CorpusManager {
    base_dir: PathBuf,
}

impl CorpusManager {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Return the corpus directory for a given contract target, creating it if absent.
    pub async fn corpus_dir(&self, contract_name: &str) -> Result<PathBuf> {
        let dir = self.base_dir.join(contract_name);
        fs::create_dir_all(&dir).await?;
        Ok(dir)
    }

    /// Seed the corpus with a set of known-interesting byte inputs.
    pub async fn seed(&self, contract_name: &str, seeds: &[Vec<u8>]) -> Result<()> {
        let dir = self.corpus_dir(contract_name).await?;
        for (i, seed) in seeds.iter().enumerate() {
            let path = dir.join(format!("seed_{:04}", i));
            fs::write(&path, seed).await?;
        }
        info!(contract = %contract_name, count = seeds.len(), "Seeded corpus");
        Ok(())
    }

    /// Generate default seeds for standard invariant patterns (zero, max, boundary values).
    pub fn default_seeds() -> Vec<Vec<u8>> {
        vec![
            // Zero amount
            0i128.to_le_bytes().to_vec(),
            // Max i128
            i128::MAX.to_le_bytes().to_vec(),
            // Typical token amount
            1_000_000i128.to_le_bytes().to_vec(),
            // Overflow boundary
            (i128::MAX - 1).to_le_bytes().to_vec(),
        ]
    }
}
