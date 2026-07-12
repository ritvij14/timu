use timu_pair::{AddressCandidate, AddressKind, CliOptions, select_address_candidates};

#[test]
fn cli_accepts_explicit_vps_connection_overrides() {
    let options = CliOptions::parse([
        "--host",
        "dev.example.com",
        "--user",
        "deploy",
        "--port",
        "2222",
    ])
    .expect("valid overrides");

    assert_eq!(options.host.as_deref(), Some("dev.example.com"));
    assert_eq!(options.username.as_deref(), Some("deploy"));
    assert_eq!(options.port, 2222);
}

#[test]
fn cli_rejects_unknown_missing_and_invalid_arguments() {
    assert!(CliOptions::parse(["--wat"]).is_err());
    assert!(CliOptions::parse(["--host"]).is_err());
    assert!(CliOptions::parse(["--port", "0"]).is_err());
    assert!(CliOptions::parse(["--port", "not-a-port"]).is_err());
}

#[test]
fn address_selection_keeps_wifi_ethernet_and_tailscale_only() {
    let candidates = select_address_candidates([
        AddressCandidate::new("en0", "192.168.1.20"),
        AddressCandidate::new("en5", "10.0.0.8"),
        AddressCandidate::new("tailscale0", "100.90.80.70"),
        AddressCandidate::new("lo0", "127.0.0.1"),
        AddressCandidate::new("docker0", "172.17.0.1"),
    ]);

    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].kind, AddressKind::Wifi);
    assert_eq!(candidates[1].kind, AddressKind::Ethernet);
    assert_eq!(candidates[2].kind, AddressKind::Tailscale);
}
