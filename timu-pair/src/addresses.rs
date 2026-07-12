use crate::System;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Wifi,
    Ethernet,
    Tailscale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressCandidate {
    pub interface: String,
    pub address: String,
    pub kind: AddressKind,
}

impl AddressCandidate {
    pub fn new(interface: &str, address: &str) -> Self {
        Self {
            interface: interface.into(),
            address: address.into(),
            kind: classify_address(interface, address).unwrap_or(AddressKind::Ethernet),
        }
    }
}

pub fn select_address_candidates<I>(candidates: I) -> Vec<AddressCandidate>
where
    I: IntoIterator<Item = AddressCandidate>,
{
    candidates
        .into_iter()
        .filter_map(|mut candidate| {
            candidate.kind = classify_address(&candidate.interface, &candidate.address)?;
            Some(candidate)
        })
        .collect()
}

/// Chooses the address the phone should use to reach this machine.
///
/// A single candidate is returned automatically. Multiple candidates are
/// presented through `prompt`, which receives the full menu text and must
/// return the user's answer.
pub fn choose_address<P>(candidates: Vec<AddressCandidate>, mut prompt: P) -> Result<String, String>
where
    P: FnMut(&str) -> Result<String, String>,
{
    if candidates.len() == 1 {
        return Ok(candidates[0].address.clone());
    }
    let mut menu = String::from("How should your phone reach this machine?\n\n");
    for (index, item) in candidates.iter().enumerate() {
        let kind = match item.kind {
            AddressKind::Wifi => "Wi-Fi",
            AddressKind::Ethernet => "Ethernet",
            AddressKind::Tailscale => "Tailscale",
        };
        menu.push_str(&format!("{}. {:<10} {}\n", index + 1, kind, item.address));
    }
    menu.push_str("\nEnter option number: ");
    let answer = prompt(&menu)?;
    let index = answer
        .trim()
        .parse::<usize>()
        .map_err(|_| "enter a valid option number".to_string())?;
    if index == 0 || index > candidates.len() {
        return Err("option number is out of range".into());
    }
    Ok(candidates[index - 1].address.clone())
}

/// Discovers Wi-Fi, Ethernet, and Tailscale address candidates using the
/// injectable [`System`] seam so tests can supply synthetic command output.
pub fn discover_addresses(system: &dyn System) -> Result<Vec<AddressCandidate>, String> {
    let mut found = if system.family() == "macos" {
        discover_macos_lan_addresses(system)
    } else {
        system
            .route_address()
            .map(|address| vec![AddressCandidate::new("eth0", &address)])
            .unwrap_or_default()
    };
    if let Ok(output) = system.command("tailscale", &["ip", "-4"])
        && output.status == 0
    {
        for line in output.stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && let Some(kind) = classify_address("tailscale0", trimmed)
            {
                found.push(AddressCandidate {
                    interface: "tailscale0".into(),
                    address: trimmed.into(),
                    kind,
                });
            }
        }
    }
    found.sort_by(|a, b| a.address.cmp(&b.address));
    found.dedup_by(|a, b| a.address == b.address);
    if found.is_empty() {
        Err("no Wi-Fi, Ethernet, or Tailscale address found; use --host".into())
    } else {
        Ok(found)
    }
}

fn discover_macos_lan_addresses(system: &dyn System) -> Vec<AddressCandidate> {
    let Ok(output) = system.command("networksetup", &["-listallhardwareports"]) else {
        return Vec::new();
    };
    if output.status != 0 {
        return Vec::new();
    }
    let mut kind = None;
    let mut found = Vec::new();
    for line in output.stdout.lines() {
        if let Some(port) = line.strip_prefix("Hardware Port: ") {
            kind = if port.contains("Wi-Fi") {
                Some(AddressKind::Wifi)
            } else if port.contains("Ethernet") {
                Some(AddressKind::Ethernet)
            } else {
                None
            };
        } else if let (Some(_kind), Some(device)) = (kind, line.strip_prefix("Device: "))
            && let Ok(address) = system.command("ipconfig", &["getifaddr", device])
            && address.status == 0
        {
            let value = address.stdout.trim().to_string();
            if !value.is_empty()
                && let Some(kind) = classify_address(device, &value)
            {
                found.push(AddressCandidate {
                    interface: device.into(),
                    address: value,
                    kind,
                });
            }
        }
    }
    found
}

fn classify_address(interface: &str, address: &str) -> Option<AddressKind> {
    let octets: Vec<u8> = address
        .split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect();
    if octets.len() != 4
        || octets[0] == 127
        || interface.starts_with("lo")
        || interface.starts_with("docker")
        || interface.starts_with("bridge")
        || interface.starts_with("veth")
    {
        return None;
    }
    if interface.to_ascii_lowercase().contains("tailscale") || is_tailscale_ipv4(&octets) {
        return Some(AddressKind::Tailscale);
    }
    if interface == "en0" || interface.starts_with("wl") {
        return Some(AddressKind::Wifi);
    }
    if interface.starts_with("en") || interface.starts_with("eth") {
        return Some(AddressKind::Ethernet);
    }
    None
}

fn is_tailscale_ipv4(octets: &[u8]) -> bool {
    octets.len() == 4 && octets[0] == 100 && (64..=127).contains(&octets[1])
}
