# Physical iOS Pairing Acceptance Checklist

This checklist records the manual evidence required once the mobile scanner/client is wired to the V1 pairing protocol. Automated Rust/npm tests prove the CLI and protocol pieces; this checklist proves the real iPhone path.

## Evidence labels

- **Automated** — already covered by repository tests or static checks.
- **Manual** — must be observed on a physical iPhone and target machine.
- **Deferred** — outside V0 or blocked until the mobile client exists.

## Mac-first acceptance path

Use this path first. It matches the first physical-device target: the developer's iPhone pairing to the developer's Mac.

### 1. Mac prerequisites

- **Manual:** iPhone and Mac are on the same reachable network, or a reachable explicit host is available.
- **Manual:** Remote Login is enabled or the CLI prompt can enable it through macOS authorization.
- **Manual:** `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub -E sha256` prints the fingerprint shown by the CLI.
- **Automated:** Declining or failing Remote Login setup exits before creating credentials (`system_boundaries`).

### 2. Start pairing

Run on the Mac:

```bash
npx timu-app
```

Optional local override if discovery is wrong:

```bash
npx timu-app --host <mac-lan-ip-or-hostname> --user <ssh-user> --port 22
```

Expected evidence:

- **Manual:** CLI prints a QR and the selected host, port, username, and host-key fingerprint.
- **Manual:** CLI does not print the temporary private key as text outside the QR.
- **Automated:** QR payload fields, expiry, and CLI argument parsing are covered by `pairing_payload` and `cli_inputs`.

### 3. Scan and compare fingerprint

On iPhone:

- Scan the QR.
- Confirm the app shows the expected host-key fingerprint from the QR.
- Let the SSH transport obtain the presented host-key fingerprint.
- Compare exact values.

Expected evidence:

- **Manual:** Matching fingerprint requires explicit trust before authentication.
- **Manual:** Mismatch blocks pairing with no bypass action.
- **Automated:** Canonical fingerprint validation is covered by `pairing_payload`; exact comparison is a mobile-client integration responsibility.

### 4. Submit device key

On iPhone after trust:

- Generate or load the device Ed25519 keypair from platform secure storage.
- Authenticate with the temporary QR credential.
- Submit only the device public key over SSH stdin.

Expected evidence:

- **Manual:** The app never logs, displays, or stores the QR temporary private key after the attempt.
- **Manual:** SSH requests no PTY and relies on the server forced command.
- **Automated:** Malformed, expired, replayed, duplicate, and oversized provisioning inputs fail without unsafe mutation (`completion_flow`, `authorized_keys`).

### 5. Confirm handoff and reconnect

Expected evidence:

- **Manual:** CLI prints success only after observing its local completion marker.
- **Manual:** App discards the temporary credential after provisional handoff.
- **Manual:** App reconnects using the permanent device key and pinned host key before saving success.
- **Manual:** Saved profile contains host, port, username, machine name, pinned fingerprint, and auth method kind only; private-key material remains in secure storage.
- **Automated:** Single-use replacement and line preservation are covered by `completion_flow` and `authorized_keys`.

### 6. Inspect cleanup

On the Mac after success:

```bash
grep 'timu-pair:' ~/.ssh/authorized_keys || true
ls -la ~/.ssh/authorized_keys
```

Expected evidence:

- **Manual:** No `timu-pair:<pairing_id>` temporary authorization remains.
- **Manual:** The permanent iPhone public key exists exactly once.
- **Manual:** Existing unrelated authorized keys remain present.
- **Manual:** File mode is safe and expected for the existing file.
- **Automated:** Cleanup, symlink rejection, unsafe-mode rejection, mode preservation, and concurrent updates are covered by `session_cleanup`, `filesystem_security`, and `concurrent_pairing`.

### 7. Readiness transition

After permanent reconnect:

- Run the readiness probe over the permanent SSH connection.
- Show `tmux`, `git`, shell, JS package managers, and agent CLI readiness.

Expected evidence:

- **Manual:** Readiness starts only after permanent-key reconnect succeeds.
- **Manual:** `tmux` missing blocks V0 session start with a copyable install command, not auto-install.
- **Automated:** Readiness itself is owned by `timu-core`; this checklist only gates the pairing-to-readiness transition.

## Negative Mac checks

Run at least once before release candidate acceptance:

- **Manual:** Scan an expired QR; app refuses before temporary auth.
- **Manual:** Start pairing, cancel on iPhone; CLI removes temporary authorization after timeout/cancel path.
- **Manual:** Trust decline returns to a safe state and does not authenticate.
- **Manual:** Host-key mismatch cannot proceed.
- **Manual:** Permanent reconnect failure does not fall back to the temporary credential.

## VPS acceptance variation

Run after the Mac-first path passes. This is the same protocol with a public or private VPS endpoint; it is not a separate flow.

### VPS command

```bash
npx timu-app --host <vps-public-dns-or-ip> --user <ssh-user> --port <ssh-port>
```

### VPS-specific checks

- **Manual:** DNS resolves to the intended VPS.
- **Manual:** Firewall/security group allows the selected SSH port from the iPhone network.
- **Manual:** Nonstandard ports work when supplied with `--port`.
- **Manual:** The QR `host`, `port`, `username`, and fingerprint match the VPS command output.
- **Manual:** Host-key mismatch still fails closed with no bypass.
- **Manual:** Permanent reconnect uses the same `host`, `port`, `username`, and pinned fingerprint.
- **Automated:** Explicit `--host`, `--user`, and `--port` parsing is covered by `cli_inputs::cli_accepts_explicit_vps_connection_overrides`.

## What remains unverified until mobile integration

- Physical camera QR scanning.
- Platform secure-storage behavior for the permanent private key.
- Mobile SSH host-key extraction and exact comparison.
- Mobile SSH temporary authentication and stdin submission.
- Permanent-key reconnect and saved profile creation.
- User-visible copy and screen flow.
- iOS backgrounding/network-loss behavior during pairing.
