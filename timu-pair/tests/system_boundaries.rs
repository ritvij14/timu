use std::cell::RefCell;
use std::time::Duration;
use timu_pair::{CommandOutput, System, ensure_ssh_available};

struct FakeSystem {
    family: &'static str,
    tcp_reachable: bool,
    prompt_response: Option<String>,
    sudo_status: i32,
    commands: RefCell<Vec<(String, Vec<String>)>>,
}

impl FakeSystem {
    fn new() -> Self {
        Self {
            family: "macos",
            tcp_reachable: false,
            prompt_response: None,
            sudo_status: 0,
            commands: RefCell::new(Vec::new()),
        }
    }

    fn with_tcp_reachable(mut self, reachable: bool) -> Self {
        self.tcp_reachable = reachable;
        self
    }

    fn with_prompt(mut self, response: &str) -> Self {
        self.prompt_response = Some(response.into());
        self
    }

    fn with_sudo_status(mut self, status: i32) -> Self {
        self.sudo_status = status;
        self
    }

    fn commands(&self) -> Vec<(String, Vec<String>)> {
        self.commands.borrow().clone()
    }
}

impl System for FakeSystem {
    fn family(&self) -> &'static str {
        self.family
    }

    fn command(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let recorded = (
            program.to_string(),
            args.iter().map(|a| a.to_string()).collect(),
        );
        self.commands.borrow_mut().push(recorded);

        let status = if program == "sudo" {
            self.sudo_status
        } else {
            0
        };
        Ok(CommandOutput {
            stdout: String::new(),
            stderr: String::new(),
            status,
        })
    }

    fn prompt(&self, _question: &str) -> Result<String, String> {
        self.prompt_response
            .clone()
            .ok_or("no prompt response configured".into())
    }

    fn now(&self) -> u64 {
        0
    }

    fn tcp_reachable(&self, _host: &str, _port: u16, _timeout: Duration) -> bool {
        self.tcp_reachable
    }

    fn route_address(&self) -> Option<String> {
        None
    }

    fn sleep(&self, _duration: Duration) {}
}

#[test]
fn ssh_listener_reachable_avoids_remote_login_setup() {
    let system = FakeSystem::new().with_tcp_reachable(true);

    ensure_ssh_available(&system, 22).expect("SSH should be considered available");

    assert!(
        system.commands().is_empty(),
        "no command runner calls should be made when the SSH listener is reachable"
    );
}

#[test]
fn remote_login_decline_makes_no_sudo_call_and_returns_actionable_failure() {
    let system = FakeSystem::new().with_prompt("no");

    let error = ensure_ssh_available(&system, 22).expect_err("declined Remote Login must fail");

    assert!(
        error.to_lowercase().contains("remote login"),
        "error should tell the user that Remote Login is required: {error}"
    );
    let commands = system.commands();
    assert!(
        commands.iter().all(|(program, _)| program != "sudo"),
        "declining Remote Login must never invoke sudo: {commands:?}"
    );
}

#[test]
fn remote_login_authorization_failure_exits_without_creating_credentials() {
    let system = FakeSystem::new().with_prompt("yes").with_sudo_status(1);

    let error = ensure_ssh_available(&system, 22)
        .expect_err("failed OS authorization for Remote Login must fail");

    assert!(
        error
            .to_lowercase()
            .contains("failed to enable remote login"),
        "error should describe the authorization failure: {error}"
    );
    let commands = system.commands();
    assert!(
        !commands.iter().any(|(program, _)| program == "ssh-keygen"),
        "failed authorization must stop before any credential-generating command: {commands:?}"
    );
}
