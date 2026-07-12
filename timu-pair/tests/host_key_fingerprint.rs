use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use timu_pair::{CommandOutput, System, host_key_fingerprint};

struct FakeSystem {
    existing_files: Vec<String>,
    command_responses: HashMap<String, CommandOutput>,
    commands: RefCell<Vec<(String, Vec<String>)>>,
}

impl FakeSystem {
    fn new() -> Self {
        Self {
            existing_files: Vec::new(),
            command_responses: HashMap::new(),
            commands: RefCell::new(Vec::new()),
        }
    }

    fn with_file(mut self, path: &str) -> Self {
        self.existing_files.push(path.to_string());
        self
    }

    fn with_command(mut self, program: &str, args: &[&str], output: CommandOutput) -> Self {
        let key = format!("{} {}", program, args.join(" "));
        self.command_responses.insert(key, output);
        self
    }
}

impl System for FakeSystem {
    fn family(&self) -> &'static str {
        "macos"
    }

    fn command(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let recorded = (
            program.to_string(),
            args.iter().map(|a| a.to_string()).collect(),
        );
        self.commands.borrow_mut().push(recorded);

        let key = format!("{} {}", program, args.join(" "));
        self.command_responses
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("unexpected command: {key}"))
    }

    fn prompt(&self, _question: &str) -> Result<String, String> {
        Err("no prompts in host key tests".into())
    }

    fn now(&self) -> u64 {
        0
    }

    fn tcp_reachable(&self, _host: &str, _port: u16, _timeout: Duration) -> bool {
        false
    }

    fn route_address(&self) -> Option<String> {
        None
    }

    fn sleep(&self, _duration: Duration) {}

    fn file_exists(&self, path: &str) -> bool {
        self.existing_files.iter().any(|f| f == path)
    }
}

const ED25519_KEY: &str = "/etc/ssh/ssh_host_ed25519_key.pub";
const ECDSA_KEY: &str = "/etc/ssh/ssh_host_ecdsa_key.pub";
const RSA_KEY: &str = "/etc/ssh/ssh_host_rsa_key.pub";

fn fingerprint_output(fingerprint: &str) -> CommandOutput {
    CommandOutput {
        stdout: format!("256 {fingerprint} no comment (ED25519)\n"),
        stderr: String::new(),
        status: 0,
    }
}

#[test]
fn ed25519_key_is_preferred_when_available() {
    let system = FakeSystem::new()
        .with_file(ED25519_KEY)
        .with_file(ECDSA_KEY)
        .with_file(RSA_KEY)
        .with_command(
            "ssh-keygen",
            &["-lf", ED25519_KEY, "-E", "sha256"],
            fingerprint_output("SHA256:aaaa"),
        );

    let fp = host_key_fingerprint(&system).expect("should read ed25519 fingerprint");

    assert_eq!(fp, "SHA256:aaaa");

    let commands = system.commands.borrow().clone();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, "ssh-keygen");
}

#[test]
fn falls_back_to_ecdsa_when_ed25519_key_does_not_exist() {
    let system = FakeSystem::new()
        .with_file(ECDSA_KEY)
        .with_file(RSA_KEY)
        .with_command(
            "ssh-keygen",
            &["-lf", ECDSA_KEY, "-E", "sha256"],
            fingerprint_output("SHA256:bbbb"),
        );

    let fp = host_key_fingerprint(&system).expect("should read ecdsa fingerprint");

    assert_eq!(fp, "SHA256:bbbb");

    let commands = system.commands.borrow().clone();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].1[1], ECDSA_KEY);
}

#[test]
fn falls_back_to_rsa_when_only_rsa_exists() {
    let system = FakeSystem::new()
        .with_file(RSA_KEY)
        .with_command(
            "ssh-keygen",
            &["-lf", RSA_KEY, "-E", "sha256"],
            fingerprint_output("SHA256:cccc"),
        );

    let fp = host_key_fingerprint(&system).expect("should read rsa fingerprint");

    assert_eq!(fp, "SHA256:cccc");
}

#[test]
fn no_host_keys_available_returns_error() {
    let system = FakeSystem::new();

    let error =
        host_key_fingerprint(&system).expect_err("no host keys should produce an error");

    assert!(error.contains("host-key fingerprint"));
}