use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use timu_pair::{
    AddressCandidate, AddressKind, CommandOutput, System, choose_address, discover_addresses,
};

struct FakeSystem {
    family: &'static str,
    tcp_reachable: bool,
    commands: RefCell<Vec<(String, Vec<String>)>>,
    command_responses: HashMap<String, CommandOutput>,
    prompt_responses: RefCell<Vec<String>>,
    route_address: Option<String>,
}

impl FakeSystem {
    fn new(family: &'static str) -> Self {
        Self {
            family,
            tcp_reachable: false,
            commands: RefCell::new(Vec::new()),
            command_responses: HashMap::new(),
            prompt_responses: RefCell::new(Vec::new()),
            route_address: None,
        }
    }

    fn with_command(mut self, program: &str, args: &[&str], output: CommandOutput) -> Self {
        let key = format!("{} {}", program, args.join(" "));
        self.command_responses.insert(key, output);
        self
    }

    fn with_route_address(mut self, address: &str) -> Self {
        self.route_address = Some(address.to_string());
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

        let key = format!("{} {}", program, args.join(" "));
        self.command_responses
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("unexpected command: {key}"))
    }

    fn prompt(&self, _question: &str) -> Result<String, String> {
        self.prompt_responses
            .borrow_mut()
            .pop()
            .ok_or("no prompt response configured".to_string())
    }

    fn now(&self) -> u64 {
        0
    }

    fn tcp_reachable(&self, _host: &str, _port: u16, _timeout: Duration) -> bool {
        self.tcp_reachable
    }

    fn route_address(&self) -> Option<String> {
        self.route_address.clone()
    }

    fn sleep(&self, _duration: Duration) {}

    fn file_exists(&self, _path: &str) -> bool {
        false
    }
}

#[test]
fn single_candidate_is_selected_without_prompt() {
    let candidates = vec![AddressCandidate::new("en0", "192.168.1.50")];

    let chosen = choose_address(candidates, |_question| {
        panic!("prompt should not be called for a single candidate")
    })
    .expect("single candidate should be chosen automatically");

    assert_eq!(chosen, "192.168.1.50");
}

#[test]
fn multiple_candidates_require_a_valid_option_number() {
    let candidates = vec![
        AddressCandidate::new("en0", "192.168.1.50"),
        AddressCandidate::new("tailscale0", "100.64.0.2"),
    ];

    let chosen = choose_address(candidates, |question| {
        assert!(
            question.contains("192.168.1.50"),
            "menu should list the first candidate: {question}"
        );
        assert!(
            question.contains("100.64.0.2"),
            "menu should list the second candidate: {question}"
        );
        assert!(
            question.contains("Enter option number"),
            "menu should ask for an option number: {question}"
        );
        Ok("2".to_string())
    })
    .expect("valid option 2 should select the second candidate");

    assert_eq!(chosen, "100.64.0.2");
}

#[test]
fn invalid_text_option_fails_without_starting_pairing() {
    let candidates = vec![
        AddressCandidate::new("en0", "192.168.1.50"),
        AddressCandidate::new("en7", "192.168.1.51"),
    ];

    let error = choose_address(candidates, |_question| Ok("abc".to_string()))
        .expect_err("non-numeric option should fail");

    assert!(
        error.to_lowercase().contains("valid option number"),
        "error should ask for a valid option number: {error}"
    );
}

#[test]
fn zero_option_fails_without_starting_pairing() {
    let candidates = vec![
        AddressCandidate::new("en0", "192.168.1.50"),
        AddressCandidate::new("en7", "192.168.1.51"),
    ];

    let error = choose_address(candidates, |_question| Ok("0".to_string()))
        .expect_err("option 0 should be out of range");

    assert!(
        error.to_lowercase().contains("out of range"),
        "error should say option is out of range: {error}"
    );
}

#[test]
fn out_of_range_option_fails_without_starting_pairing() {
    let candidates = vec![
        AddressCandidate::new("en0", "192.168.1.50"),
        AddressCandidate::new("en7", "192.168.1.51"),
    ];

    let error = choose_address(candidates, |_question| Ok("3".to_string()))
        .expect_err("option 3 should be out of range for two candidates");

    assert!(
        error.to_lowercase().contains("out of range"),
        "error should say option is out of range: {error}"
    );
}

#[test]
fn macos_networksetup_parsing_extracts_wifi_and_ethernet() {
    let system = FakeSystem::new("macos").with_command(
        "networksetup",
        &["-listallhardwareports"],
        CommandOutput {
            stdout: concat!(
                "Hardware Port: Wi-Fi\n",
                "Device: en0\n",
                "Ethernet Address: ab:cd:ef:12:34:56\n\n",
                "Hardware Port: Ethernet\n",
                "Device: en7\n",
                "Ethernet Address: ab:cd:ef:12:34:57\n\n",
                "Hardware Port: Thunderbolt Bridge\n",
                "Device: bridge0\n",
                "Ethernet Address: ab:cd:ef:12:34:58\n\n",
            )
            .to_string(),
            stderr: String::new(),
            status: 0,
        },
    );

    let mut system = system;
    system.command_responses.insert(
        "ipconfig getifaddr en0".to_string(),
        CommandOutput {
            stdout: "192.168.1.50\n".to_string(),
            stderr: String::new(),
            status: 0,
        },
    );
    system.command_responses.insert(
        "ipconfig getifaddr en7".to_string(),
        CommandOutput {
            stdout: "192.168.1.51\n".to_string(),
            stderr: String::new(),
            status: 0,
        },
    );
    system.command_responses.insert(
        "ipconfig getifaddr bridge0".to_string(),
        CommandOutput {
            stdout: String::new(),
            stderr: String::new(),
            status: 1,
        },
    );

    let found = discover_addresses(&system).expect("macOS discovery should succeed");

    assert_eq!(found.len(), 2);
    let wifi = found
        .iter()
        .find(|c| c.address == "192.168.1.50")
        .expect("Wi-Fi address");
    assert_eq!(wifi.kind, AddressKind::Wifi);
    let ethernet = found
        .iter()
        .find(|c| c.address == "192.168.1.51")
        .expect("Ethernet address");
    assert_eq!(ethernet.kind, AddressKind::Ethernet);

    let commands = system.commands();
    assert!(
        commands
            .iter()
            .any(|(p, a)| p == "networksetup" && a == &["-listallhardwareports"])
    );
    assert!(
        commands
            .iter()
            .any(|(p, a)| p == "ipconfig" && a == &["getifaddr", "en0"])
    );
    assert!(
        commands
            .iter()
            .any(|(p, a)| p == "ipconfig" && a == &["getifaddr", "en7"])
    );
}

#[test]
fn tailscale_discovery_adds_valid_ipv4_candidates() {
    let system = FakeSystem::new("linux")
        .with_route_address("10.0.0.5")
        .with_command(
            "tailscale",
            &["ip", "-4"],
            CommandOutput {
                stdout: "100.64.1.2\n".to_string(),
                stderr: String::new(),
                status: 0,
            },
        );

    let found = discover_addresses(&system).expect("discovery should succeed");

    assert_eq!(found.len(), 2);
    let tailscale = found
        .iter()
        .find(|c| c.address == "100.64.1.2")
        .expect("Tailscale address");
    assert_eq!(tailscale.kind, AddressKind::Tailscale);
    let route = found
        .iter()
        .find(|c| c.address == "10.0.0.5")
        .expect("route address");
    assert_eq!(route.kind, AddressKind::Ethernet);
}

#[test]
fn loopback_and_docker_interfaces_are_excluded() {
    let system = FakeSystem::new("macos").with_command(
        "networksetup",
        &["-listallhardwareports"],
        CommandOutput {
            stdout: concat!(
                "Hardware Port: Loopback\n",
                "Device: lo0\n",
                "Ethernet Address: \n\n",
                "Hardware Port: Docker\n",
                "Device: docker0\n",
                "Ethernet Address: ab:cd:ef:12:34:56\n\n",
                "Hardware Port: Wi-Fi\n",
                "Device: en0\n",
                "Ethernet Address: ab:cd:ef:12:34:57\n\n",
            )
            .to_string(),
            stderr: String::new(),
            status: 0,
        },
    );

    let mut system = system;
    system.command_responses.insert(
        "ipconfig getifaddr lo0".to_string(),
        CommandOutput {
            stdout: "127.0.0.1\n".to_string(),
            stderr: String::new(),
            status: 0,
        },
    );
    system.command_responses.insert(
        "ipconfig getifaddr docker0".to_string(),
        CommandOutput {
            stdout: "172.17.0.1\n".to_string(),
            stderr: String::new(),
            status: 0,
        },
    );
    system.command_responses.insert(
        "ipconfig getifaddr en0".to_string(),
        CommandOutput {
            stdout: "192.168.1.50\n".to_string(),
            stderr: String::new(),
            status: 0,
        },
    );

    let found = discover_addresses(&system).expect("macOS discovery should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].address, "192.168.1.50");
    assert_eq!(found[0].kind, AddressKind::Wifi);
}

#[test]
fn malformed_tailscale_output_is_ignored() {
    let system = FakeSystem::new("linux")
        .with_route_address("10.0.0.5")
        .with_command(
            "tailscale",
            &["ip", "-4"],
            CommandOutput {
                stdout: "not-an-address\n".to_string(),
                stderr: String::new(),
                status: 0,
            },
        );

    let found = discover_addresses(&system).expect("discovery should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].address, "10.0.0.5");
}
