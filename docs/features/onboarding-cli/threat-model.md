# Onboarding Pairing Threat Model

This is the V0 threat model for the `npx timu` permanent-key pairing flow. It covers the local CLI, OpenSSH temporary authorization, QR payload, and future mobile client contract. It does not add hosted services, telemetry, cloud sync, or mobile UI architecture.

## Assets

- User SSH account on the target machine.
- Existing `~/.ssh/authorized_keys` entries.
- Temporary QR private key and matching temporary `authorized_keys` line.
- Device-generated permanent private key in platform secure storage.
- SSH host-key fingerprint used to bind the QR to the intended machine.
- Pairing completion marker and temporary CLI artifacts.
- Saved machine profile metadata after successful permanent-key reconnect.

## Trust boundaries

- QR display crosses from the target machine terminal to the phone camera.
- SSH crosses from phone network to target `host:port`.
- Host-key comparison crosses from untrusted network evidence to user trust.
- Temporary SSH authentication crosses into OpenSSH forced-command execution.
- Forced-command stdin crosses from the app into `authorized_keys` mutation.
- Filesystem mutation crosses from ceremony-local artifacts to durable SSH authorization.
- Permanent-key reconnect crosses from provisional handoff to saved-machine success.

## Adversaries

- Nearby observer or screenshot recipient who sees the QR.
- Network attacker who can redirect DNS/IP traffic or present another SSH host key.
- Malicious or buggy app client that submits malformed public-key input.
- Local process racing or interrupting pairing on the target account.
- Local attacker who can replace `.ssh` or `authorized_keys` with symlinks or unsafe files.
- User or automation that replays a stale, consumed, or duplicated pairing credential.
- Compromised release/download path for the npm wrapper or native binary.

## Threats and controls

| Threat | Impact | V0 control | Evidence | Residual risk |
| --- | --- | --- | --- | --- |
| QR theft before expiry | Attacker can attempt temporary authentication. | QR credential expires after five minutes, temporary authorization is restricted to one forced command, and cleanup removes the tagged line on timeout/cancel/setup failure. | `pairing_payload::qr_payload_rejects_expired_credentials`, `completion_flow::scanning_an_expired_qr_cannot_change_ssh_access`, `session_cleanup::*`, protocol §1/§6. | A stolen unexpired QR can pair if the attacker also reaches the host and the user trusts the shown fingerprint. Keep QR visible only during setup. |
| QR replay after success | A captured QR installs another key later. | Successful handoff removes the unique tagged temporary line; missing or duplicate matching lines fail closed. | `completion_flow::a_pairing_credential_is_single_use_and_cannot_install_a_second_key`, `authorized_keys::*duplicate*`, protocol §6. | SSH server logs may still show failed replay attempts; V0 does not add alerting. |
| Host-key substitution | Phone pairs with the wrong machine. | QR carries the expected OpenSSH SHA-256 host-key fingerprint; app contract requires exact comparison and explicit trust with no bypass. | `pairing_payload::qr_payload_rejects_zero_port_invalid_pairing_id_and_noncanonical_host_fingerprint`, protocol §3.2/§8, AGENTS hard block: never disable host-key verification. | Until mobile integration exists, exact UI and secure pin persistence are unverified manual/client work. |
| Malicious public-key input | Forced command writes options, multiline data, private material, or shell syntax into authorization. | Forced command reads bounded stdin, validates exactly one single-line `ssh-ed25519` public key, never invokes a shell, and writes the submitted key only after validation. | `completion_flow::malformed_device_keys_fail_without_removing_temporary_access`, `authorized_keys::malformed_device_key_is_rejected_without_changing_authorizations`, protocol §3.3/§4. | V0 accepts a public-key comment as metadata; clients must not treat the comment as identity. |
| Interrupted provisioning | Temporary authorization or local artifacts remain after Ctrl-C, timeout, startup failure, or Remote Login failure. | `CleanupGuard` removes only this ceremony's tagged authorization and temp root on terminal exits before successful handoff. | `session_cleanup::*`, `system_boundaries::*`, protocol §6. | A process killed with `SIGKILL` cannot run Rust destructors; user can rerun setup or inspect `authorized_keys`. |
| Symlinked `.ssh` or `authorized_keys` | Pairing overwrites an attacker-chosen file. | Every mutation rejects symlinked parent directory or target file before writing, and revalidates after acquiring the lock. | `filesystem_security::pairing_rejects_a_symlinked_authorized_keys_file_without_modifying_its_target`, `filesystem_security::pairing_rejects_a_symlinked_ssh_directory_without_modifying_its_target`, protocol §10. | V0 does not prove kernel-level protection against every same-user TOCTOU beyond revalidation and same-directory atomic rename. |
| Unsafe authorization-file mode | Another local user/group can alter SSH access during pairing. | Existing group/world-writable `authorized_keys` is rejected; CLI-created `.ssh` and `authorized_keys` are tightened to `0700`/`0600`; atomic handoff preserves an already-safe existing mode. | `filesystem_security::pairing_rejects_group_or_world_writable_authorized_keys`, `filesystem_security::pairing_handoff_preserves_authorized_keys_permissions`, protocol §4/§10. | V0 does not inspect file owner across platforms; ownership problems surface as install failure guidance. |
| Concurrent pairing race | Two ceremonies lose unrelated keys or consume each other's temporary line. | Authorization mutations use an adjacent lock file, re-read current contents under lock, and atomically replace only the matching tagged line. | `concurrent_pairing::*`, protocol §4/§6/§10. | Locking is per target directory and account; V0 does not coordinate across nonstandard external mutation tools. |
| Compromised release artifact | User runs a tampered pairing binary. | npm wrapper release path must checksum native binaries and fail closed on unsupported/corrupt downloads. | Phase 2 tests in `timu-npx/`, plan completion criterion, Phase 6 release verification. | Users can still bypass npm verification by running arbitrary binaries outside the documented install path. |
| Secret persistence in app profile | SSH secrets leak through SQLite/profile serialization. | Pairing result/profile contract stores only connection metadata and auth method kind; private key material remains in secure storage. | Pairing protocol §7, AGENTS hard block: never persist SSH secrets in `MachineProfile` or SQLite. | Native secure-storage implementation remains outside `timu-pair` and must be verified in mobile integration. |

## Review checklist

Before marking pairing integration complete:

- Every app path that consumes the QR redacts `temporary_credential` in logs, debug output, crash reports, and errors.
- Host-key mismatch has no continue/bypass action.
- Permanent success is returned only after reconnecting with the device key and the pinned host key.
- Any new public error code maps to a distinct user corrective action.
- Any change to filesystem mutation keeps symlink rejection, unsafe-mode rejection, line preservation, and atomic replacement covered by tests.
