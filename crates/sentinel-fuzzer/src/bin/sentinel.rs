//! Local CI entry point: exit 0 on a clean dynamic check, 1 on findings, 2 on errors.
use sentinel_core::config::SentinelConfig;
use sentinel_fuzzer::{
    analysis::analyze,
    workspace::{snapshot, source_identity},
};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(failed) => std::process::exit(i32::from(failed)),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(2);
        }
    }
}

async fn run() -> anyhow::Result<bool> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "usage: sentinel <config.toml> <git-workspace-root>"
    );
    let config = SentinelConfig::from_toml(&std::fs::read_to_string(&args[0])?)?;
    config.validate()?;
    let root = PathBuf::from(&args[1]);
    let (repository, commit) = source_identity(&root).await?;
    let checkout = snapshot(&root, &repository, &commit).await?;
    let run_id = uuid::Uuid::new_v4();
    let mut results = Vec::new();
    for contract in &config.contracts {
        results.push(analyze(&checkout.root(), run_id, contract).await?);
    }
    let failed = results.iter().any(|result| !result.findings.is_empty());
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": run_id, "repository": repository, "source_commit": commit,
            "status": if failed { "failed" } else { "passed" },
            "scope": "dynamic analysis; not a formal proof", "results": results,
        }))?
    );
    Ok(failed)
}
