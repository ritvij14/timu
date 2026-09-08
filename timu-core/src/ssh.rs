//! SSH transport abstraction — the seam that lets every SSH-dependent flow be
//! tested with no network.
//!
//! [`SshTransport::run_command`] is the only capability V0 readiness needs. Real
//! SFTP / interactive PTY come later as separate traits on the same connection.
//! The trait is generic (not `dyn`) so we use native async-fn-in-traits with no
//! `async-trait` dependency; `TimuCore` will be parameterized over `T:
//! SshTransport` (real = `RusshSshTransport`, tests = [`FakeSshTransport`]).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

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

/// `Arc`-sharing: the FFI layer holds the connection's transport behind an
/// `Arc` so the pane watcher can clone it and stream on the same connection.
impl<T: SshTransport + ?Sized> SshTransport for std::sync::Arc<T> {
    async fn run_command(&self, command: &str) -> Result<CommandOutput, TimuError> {
        (**self).run_command(command).await
    }
}

/// In-memory, scriptable transport for tests. Panics-free: looks up the exact
/// command in its script table; unscripted commands return [`TimuError::Other`]
/// so tests fail loudly on unexpected probes rather than silently passing.
#[derive(Debug, Default)]
pub struct FakeSshTransport {
    scripts: HashMap<String, Result<CommandOutput, TimuError>>,
    /// Sequential outputs for commands whose response changes over time
    /// (e.g. successive pane captures in the streaming watcher). Plays in
    /// order, then sticks on the last output.
    sequences: Mutex<HashMap<String, VecDeque<CommandOutput>>>,
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

    /// Script a command to return successive outputs on consecutive calls;
    /// after the list is exhausted, the last output repeats.
    pub fn script_sequence(&mut self, command: impl Into<String>, outputs: Vec<CommandOutput>) {
        self.sequences
            .lock()
            .expect("fake sequence mutex")
            .insert(command.into(), outputs.into());
    }
}

impl SshTransport for FakeSshTransport {
    async fn run_command(&self, command: &str) -> Result<CommandOutput, TimuError> {
        if let Some(queue) = self
            .sequences
            .lock()
            .expect("fake sequence mutex")
            .get_mut(command)
        {
            if queue.len() > 1 {
                return Ok(queue.pop_front().expect("non-empty queue"));
            }
            return Ok(queue.front().expect("non-empty queue").clone());
        }
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
    async fn scripted_sequence_plays_outputs_in_order_then_sticks_on_the_last() {
        let mut fake = FakeSshTransport::new();
        fake.script_sequence(
            "poll",
            vec![CommandOutput::success("one"), CommandOutput::success("two")],
        );

        assert_eq!(fake.run_command("poll").await.unwrap().stdout, "one");
        assert_eq!(fake.run_command("poll").await.unwrap().stdout, "two");
        assert_eq!(fake.run_command("poll").await.unwrap().stdout, "two");
        assert_eq!(fake.run_command("poll").await.unwrap().stdout, "two");
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

    #[tokio::test]
    async fn arc_wrapped_transport_dispatches_run_command() {
        // The FFI layer clones an Arc<T> into the pane watcher so streaming
        // shares the connection's transport. Generic bind defeats deref
        // coercion so T really is Arc<FakeSshTransport>.
        async fn call_through<T: SshTransport>(transport: T) -> Result<CommandOutput, TimuError> {
            transport.run_command("uname").await
        }

        let mut fake = FakeSshTransport::new();
        fake.script_success("uname", "Linux\n");
        let shared = std::sync::Arc::new(fake);
        let out = call_through(shared.clone()).await.expect("scripted ok");
        assert_eq!(out.stdout, "Linux\n");
    }

    #[test]
    fn command_output_success_helper_has_zero_exit_and_empty_stderr() {
        let out = CommandOutput::success("hi");
        assert_eq!(out.exit_code, 0);
        assert!(out.stderr.is_empty());
        assert_eq!(out.stdout, "hi");
    }
}
