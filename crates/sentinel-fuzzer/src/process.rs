//! Bounded subprocess execution shared by fuzzing, regression tests and Git.
use anyhow::{Context, Result};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const MAX_OUTPUT: u64 = 4 * 1024 * 1024;

#[cfg(unix)]
struct ProcessGroup(u32);
#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // The child starts a new group. Kill that group, including grandchildren,
        // on timeout, cancellation, output overflow, or normal completion.
        unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL) };
    }
}

pub async fn output(command: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .context("could not spawn analysis command")?;
    #[cfg(unix)]
    let _group = ProcessGroup(child.id().context("missing subprocess id")?);
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;
    let collect = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let read_out = async {
            stdout.take(MAX_OUTPUT + 1).read_to_end(&mut out).await?;
            anyhow::ensure!(
                out.len() as u64 <= MAX_OUTPUT,
                "stdout exceeded analysis output limit"
            );
            Ok::<_, anyhow::Error>(())
        };
        let read_err = async {
            stderr.take(MAX_OUTPUT + 1).read_to_end(&mut err).await?;
            anyhow::ensure!(
                err.len() as u64 <= MAX_OUTPUT,
                "stderr exceeded analysis output limit"
            );
            Ok::<_, anyhow::Error>(())
        };
        let (status, _, _) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(child.wait().await?) },
            read_out,
            read_err
        )?;
        Ok::<_, anyhow::Error>(std::process::Output {
            status,
            stdout: out,
            stderr: err,
        })
    };
    tokio::time::timeout(timeout, collect)
        .await
        .context("analysis command timed out")?
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn enforces_timeout_and_output_limits() {
        assert!(
            output(Command::new("sleep").arg("10"), Duration::from_millis(20))
                .await
                .is_err()
        );
        assert!(output(
            Command::new("head").args(["-c", "5000000", "/dev/zero"]),
            Duration::from_secs(5)
        )
        .await
        .is_err());
    }
}
