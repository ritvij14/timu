# Onboarding CLI

> `npx timu-app` is the one-time SSH pairing ceremony for the timu mobile app. The canonical product flow is `docs/prds/v0-prd.md` §6; project security rules are in `AGENTS.md` §2.

## 1. What this owns

- `timu-npx/` — npm distribution and platform-binary launcher exposed as `npx timu-app`
- `timu-pair/` — native macOS/Linux pairing CLI
- SSH availability checks and an explicit macOS Remote Login enablement offer
- Wi-Fi, Ethernet, and Tailscale address discovery/selection; `--host` override for VPS targets
- Five-minute restricted ephemeral SSH credential
- Versioned QR pairing payload
- SSH host-key fingerprint display and transport to the app
- Permanent device-public-key handoff and deterministic temporary-artifact cleanup

It does not own mobile screens, permanent private-key storage, readiness probing, folder selection, or tmux sessions. In V0, readiness runs only in the app after pairing.

## 2. User flow

1. User runs `npx timu-app`.
2. CLI verifies SSH availability. On macOS it may explicitly offer to enable Remote Login through the OS authorization flow.
3. CLI discovers supported addresses. One candidate is automatic; multiple candidates are selected by entering an option number.
4. CLI creates a five-minute restricted pairing credential and prints the SSH host-key fingerprint beside a QR.
5. App scans the QR, displays the same fingerprint, and requires explicit trust.
6. App connects using the temporary credential and installs its permanent public key through the restricted provisioning command.
7. Temporary authorization and key files are removed on success, timeout, interruption, or failure.
8. CLI confirms pairing and exits. The app reconnects with its permanent key and runs readiness.

## 3. Pairing contract

Detailed supporting docs:

- [`onboarding-cli/pairing-protocol-v1.md`](onboarding-cli/pairing-protocol-v1.md) — language-neutral V1 protocol and app-facing error contract.
- [`onboarding-cli/threat-model.md`](onboarding-cli/threat-model.md) — V0 pairing assets, trust boundaries, threats, controls, and residual risks.
- [`onboarding-cli/physical-ios-acceptance.md`](onboarding-cli/physical-ios-acceptance.md) — Mac-first physical iPhone acceptance checklist plus VPS variation.

The QR payload is versioned and contains only what the app needs for the one-time ceremony:

- protocol version
- machine hostname
- SSH host, port, and username
- expected SSH host-key fingerprint
- expiry timestamp
- pairing identifier
- ephemeral private key

The payload must reject unsupported versions, malformed fields, invalid ports, and expired credentials. It must never be logged by the CLI.

The temporary `authorized_keys` entry is tagged with the pairing identifier and restricted to the key-install operation. Port, agent, and X11 forwarding plus PTY allocation are disabled. The provisioning operation accepts exactly one valid SSH public key, installs a tagged permanent entry atomically, removes the temporary entry, and cannot execute arbitrary shell commands.

## 4. Security rules

- Never auto-accept or disable SSH host-key verification.
- Never persist the ephemeral private key outside a permission-restricted temporary directory; delete it on every exit path.
- Never print the QR payload or private key as text.
- Never place permanent private-key material on the machine; it is generated and stored on the iPhone.
- Never silently enable Remote Login or weaken sshd configuration.
- Never overwrite unrelated `authorized_keys` entries.
- A timeout or interrupted CLI must remove its own temporary entry and files.

## 5. Testing

Strict TDD applies. Tests are requirements-driven, not derived from the existing implementation.

- Rust unit tests: payload validation, argument parsing, address classification, public-key validation, authorized-key transformations, expiry.
- Rust integration tests: isolated temporary-file and subprocess coverage for pairing handoff; no real user SSH configuration is touched.
- Node tests: platform asset selection, launcher argument/exit propagation, download failures, and integrity checks.
- Package smoke test: `npm pack`, install the tarball in a temporary directory, and execute the packaged launcher against a controlled fake binary.
- Manual acceptance after automation: run `npx timu-app` on macOS, scan from a physical iPhone, compare fingerprints, provision the device key, reconnect, and validate the same flow against a VPS target.

Current automated coverage includes Rust tests for payload round-trips; expiry-boundary, version, unknown-field, and fixture validation; CLI overrides and address classification; temporary-key restrictions; line-preserving permanent-key handoff; injection/missing-ID rejection; expired-QR rejection; malformed-device-key rejection; single-use enforcement; symlinked `.ssh`/`authorized_keys` rejection; unsafe `authorized_keys` modes; cleanup after startup failure and cancellation; concurrent authorization mutation; and the hidden completion subprocess against temporary files. Node tests cover launcher argument/exit-code propagation and missing-binary recovery; unsupported platforms; download failure cleanup; checksum mismatch cleanup and verified-download permissions; plus `npm pack`, temporary installation, and packaged-launcher smoke execution. These automated tests do not perform physical iPhone pairing, macOS Remote Login enablement, or VPS acceptance; those remain manual acceptance work.

Commands:

- `cd timu-pair && cargo test`
- `cd timu-pair && cargo clippy --all-targets -- -D warnings`
- `cd timu-npx && npm test`
