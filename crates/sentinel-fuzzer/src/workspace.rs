//! Execute tracked source from an exact local Git commit in a disposable clone.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;
use tokio::process::Command;

pub struct Snapshot {
    directory: TempDir,
    pub commit: String,
    pub repository: String,
}

impl Snapshot {
    pub fn root(&self) -> PathBuf {
        self.directory.path().join("source")
    }
}

async fn git(root: &Path, args: &[&str]) -> Result<String> {
    let result = crate::process::output(
        Command::new("git").arg("-C").arg(root).args(args),
        Duration::from_secs(60),
    )
    .await?;
    anyhow::ensure!(
        result.status.success(),
        "Git source validation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

pub fn repository_name(remote: &str) -> Result<String> {
    let path = if let Some((_, path)) = remote.split_once("://") {
        path.split_once('/')
            .context("remote URL has no repository path")?
            .1
    } else {
        remote
            .split_once(':')
            .context("expected a hosted Git remote")?
            .1
    };
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let parts: Vec<_> = path.split('/').collect();
    anyhow::ensure!(
        parts.len() == 2
            && parts.iter().all(|s| !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))),
        "expected owner/repository remote"
    );
    Ok(path.to_owned())
}

pub async fn source_identity(root: &Path) -> Result<(String, String)> {
    let root = root.canonicalize()?;
    let top = PathBuf::from(git(&root, &["rev-parse", "--show-toplevel"]).await?).canonicalize()?;
    anyhow::ensure!(
        root == top,
        "workspace must be the Git repository root, not a child of another repository"
    );
    let commit = git(&root, &["rev-parse", "HEAD"]).await?;
    let remote = git(&root, &["remote", "get-url", "origin"]).await?;
    Ok((repository_name(&remote)?, commit))
}

pub async fn snapshot(root: &Path, repository: &str, commit: &str) -> Result<Snapshot> {
    anyhow::ensure!(
        commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "commit_sha must be a full 40-character Git SHA"
    );
    let (actual_repository, _) = source_identity(root).await?;
    anyhow::ensure!(
        actual_repository.eq_ignore_ascii_case(repository),
        "submitted repository does not match configured workspace origin"
    );
    let resolved = git(
        root,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )
    .await?;
    anyhow::ensure!(
        resolved.eq_ignore_ascii_case(commit),
        "commit is not an exact commit object"
    );
    let directory = tempfile::Builder::new().prefix("sentinel-run-").tempdir()?;
    let destination = directory.path().join("source");
    let cloned = crate::process::output(
        Command::new("git")
            .args([
                "clone",
                "--local",
                "--no-hardlinks",
                "--no-checkout",
                "--quiet",
                "--",
            ])
            .arg(root.canonicalize()?)
            .arg(&destination),
        Duration::from_secs(60),
    )
    .await?;
    anyhow::ensure!(
        cloned.status.success(),
        "could not create isolated source checkout"
    );
    git(
        &destination,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--quiet",
            "--detach",
            commit,
        ],
    )
    .await?;
    Ok(Snapshot {
        directory,
        commit: resolved,
        repository: actual_repository,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_hosted_remotes_without_accepting_local_paths() {
        assert_eq!(
            repository_name("git@github-marvin:Org/Repo.git").unwrap(),
            "Org/Repo"
        );
        assert_eq!(
            repository_name("https://github.com/Org/Repo.git").unwrap(),
            "Org/Repo"
        );
        assert!(repository_name("/home/user/repository").is_err());
    }
    #[tokio::test]
    async fn pins_committed_content_and_rejects_spoofed_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        git(root, &["init", "--quiet"]).await.unwrap();
        git(
            root,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/test/token.git",
            ],
        )
        .await
        .unwrap();
        std::fs::write(root.join("source.txt"), "committed").unwrap();
        git(root, &["add", "source.txt"]).await.unwrap();
        git(
            root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        )
        .await
        .unwrap();
        let (_, sha) = source_identity(root).await.unwrap();
        std::fs::write(root.join("source.txt"), "uncommitted change").unwrap();
        let copy = snapshot(root, "test/token", &sha).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(copy.root().join("source.txt")).unwrap(),
            "committed"
        );
        assert!(snapshot(root, "wrong/repo", &sha).await.is_err());
        assert!(snapshot(root, "test/token", "abc").await.is_err());
        let child = root.join("nested");
        std::fs::create_dir(&child).unwrap();
        assert!(source_identity(&child).await.is_err());
    }
}
