//! SSH transport abstraction — the seam that lets every SSH-dependent flow be
//! tested with no network.
//!
//! [`SshTransport::run_command`] is the only capability V0 readiness needs. Real
//! SFTP / interactive PTY come later as separate traits on the same connection.
//! The trait is generic (not `dyn`) so we use native async-fn-in-traits with no
//! `async-trait` dependency; `TimuCore` will be parameterized over `T:
//! SshTransport` (real = `RusshSshTransport`, tests = [`FakeSshTransport`]).

use std::collections::HashMap;

use crate::error::TimuError;

/// Result of running one shell command over SSH.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CommandOutput {
    pub fn new(stdout: impl Into<String>, stderr: impl Into<String>, exit_code: i32) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
        }
    }

    /// Convenience for a successful command with empty stderr.
    pub fn success(stdout: impl Into<String>) -> Self {
        Self::new(stdout, String::new(), 0)
    }
}

/// Run a shell command on the target machine.
///
/// We use native `async fn` in the trait and parameterize `TimuCore` over `T:
/// SshTransport` (generics, not `dyn`) — see ADR-004. The `async_fn_in_trait`
/// lint is allowed because we never need `dyn` dispatch; if we later do, we'll
/// desugar to `impl Future + Send` at that point.
#[allow(async_fn_in_trait)]
pub trait SshTransport: Send + Sync {
    async fn run_command(&self, command: &str) -> Result<CommandOutput, TimuError>;
}

/// In-memory, scriptable transport for tests. Panics-free: looks up the exact
/// command in its script table; unscripted commands return [`TimuError::Other`]
/// so tests fail loudly on unexpected probes rather than silently passing.
#[derive(Debug, Default, Clone)]
pub struct FakeSshTransport {
    scripts: HashMap<String, Result<CommandOutput, TimuError>>,
}

impl FakeSshTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Script a command to return a successful output.
    pub fn script_success(&mut self, command: impl Into<String>, stdout: impl Into<String>) {
        self.scripts
            .insert(command.into(), Ok(CommandOutput::success(stdout)));
    }

    /// Script a command to return a specific output.
    pub fn script(&mut self, command: impl Into<String>, output: CommandOutput) {
        self.scripts.insert(command.into(), Ok(output));
    }

    /// Script a command to fail with a typed error.
    pub fn script_error(&mut self, command: impl Into<String>, error: TimuError) {
        self.scripts.insert(command.into(), Err(error));
    }
}

impl SshTransport for FakeSshTransport {
    async fn run_command(&self, command: &str) -> Result<CommandOutput, TimuError> {
        match self.scripts.get(command) {
            Some(result) => result.clone(),
            None => Err(TimuError::Other(format!(
                "fake: unscripted command: {command}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readiness_probe::{build_probe_command, parse_probe_output};

    #[tokio::test]
    async fn returns_scripted_success_output() {
        let mut fake = FakeSshTransport::new();
        fake.script_success("uname -a", "Linux box 6.8.0\n");
        let out = fake.run_command("uname -a").await.expect("scripted ok");
        assert_eq!(out.stdout, "Linux box 6.8.0\n");
        assert_eq!(out.exit_code, 0);
    }

    #[tokio::test]
    async fn returns_scripted_error() {
        let mut fake = FakeSshTransport::new();
        fake.script_error("boom", TimuError::PortUnreachable);
        let err = fake.run_command("boom").await.expect_err("should fail");
        assert_eq!(err, TimuError::PortUnreachable);
        assert_eq!(err.code(), "port_unreachable");
    }

    #[tokio::test]
    async fn unscripted_command_returns_other_error() {
        let fake = FakeSshTransport::new();
        let err = fake.run_command("nope").await.expect_err("should fail");
        assert_eq!(err.code(), "other");
        assert!(err.to_string().contains("unscripted"));
    }

    #[tokio::test]
    async fn readiness_flow_end_to_end_with_fake_transport() {
        // Path: readiness flow is run through the trait, then parsed.
        let mut fake = FakeSshTransport::new();
        // Simulate a machine missing tmux + claude, everything else ready.
        let mut simulated = String::new();
        for t in crate::readiness::Tool::all() {
            let status = match t {
                crate::readiness::Tool::Tmux | crate::readiness::Tool::Claude => "missing",
                _ => "ready",
            };
            simulated.push_str(t.as_str());
            simulated.push(':');
            simulated.push_str(status);
            simulated.push('\n');
        }
        fake.script_success(build_probe_command(), simulated);

        let out = fake
            .run_command(&build_probe_command())
            .await
            .expect("probe runs");
        let report = parse_probe_output(&out.stdout);

        assert!(report.tmux_is_missing());
        assert_eq!(
            report.get(crate::readiness::Tool::Claude),
            crate::readiness::ToolStatus::Missing
        );
        assert_eq!(
            report.get(crate::readiness::Tool::Git),
            crate::readiness::ToolStatus::Ready
        );
    }

    #[test]
    fn command_output_success_helper_has_zero_exit_and_empty_stderr() {
        let out = CommandOutput::success("hi");
        assert_eq!(out.exit_code, 0);
        assert!(out.stderr.is_empty());
        assert_eq!(out.stdout, "hi");
    }
}