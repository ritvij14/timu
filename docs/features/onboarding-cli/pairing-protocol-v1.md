# Permanent-Key Pairing Protocol V1

This document defines the language-neutral contract between the `npx timu` pairing CLI, the target machine's OpenSSH server, and the timu app. It covers only the one-time handoff from a temporary pairing credential to a device-generated permanent SSH key. Readiness checks and app screen design are outside this contract.

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

## 1. Security invariants

- The device MUST generate the permanent Ed25519 keypair. Its private key MUST remain in platform secure storage and MUST NOT be sent to the target machine or included in this protocol's request/result values.
- The QR's temporary private key is authentication material, not identity material. It MUST be held only for the pairing attempt, MUST NOT be logged or rendered as text, and MUST be discarded on every terminal outcome.
- The app MUST verify the server host key against the QR fingerprint and require explicit user trust before temporary authentication. A mismatch MUST fail closed.
- The temporary SSH authorization MUST allow exactly one valid permanent public-key installation. It MUST NOT provide a shell, PTY, forwarding, or arbitrary command execution.
- The handoff MUST replace only the matching temporary authorization entry, atomically, without changing unrelated `authorized_keys` entries.
- Pairing expires five minutes after CLI setup and is single-use. Expired or consumed credentials MUST NOT change SSH access.

## 2. QR envelope and payload

The QR value is:

```text
timu://pair?data=<base64url-no-padding(UTF-8 JSON)>
```

V1 JSON uses the exact field names below. All fields are required, unknown fields MUST be rejected, and a receiver MUST reject a `version` it does not support.

| Field | V1 type | Stable semantics |
| --- | --- | --- |
| `version` | integer | Protocol version. Exactly `1` for this document. It is not an app or CLI release version. |
| `pairing_id` | string | Opaque identifier for one pairing ceremony and its tagged temporary authorization. V1 producers use only ASCII letters, digits, `-`, and `_`; consumers MUST NOT infer timestamps, process IDs, or identity from it. |
| `machine_name` | string | Non-empty hostname-derived display-name suggestion. It is not an SSH endpoint and is not trusted identity evidence. |
| `host` | string | Non-empty hostname or IP address the device should dial. It may represent LAN, Tailscale, DNS, or an explicit VPS override. |
| `port` | integer | SSH TCP port in `1..=65535`. |
| `username` | string | Non-empty SSH account name used for temporary and permanent authentication. |
| `host_key_fingerprint` | string | Expected fingerprint of the target sshd Ed25519 host public key, in OpenSSH SHA-256 display form (`SHA256:<base64>`). Comparison is an exact comparison of the canonical fingerprint value, not a visual similarity check. |
| `expires_at_unix` | integer | Absolute UTC Unix time in seconds. The payload is expired when `now >= expires_at_unix`. CLI setup sets it to five minutes after creation. |
| `ephemeral_private_key` | string | Temporary unencrypted OpenSSH Ed25519 private-key serialization used only for this pairing authentication. It MUST be treated as secret payload content. |

The app MUST reject a wrong URI prefix, invalid base64url or JSON, a missing or wrongly typed required field, an empty required string, an invalid port, an unsupported version, or an expired payload before changing local trust or remote authorization. Implementations SHOULD apply reasonable QR and field-size bounds before decoding or allocation.

## 3. App-facing state and operations

These shapes define stable semantic boundaries, not a frontend API or screen design. Language bindings MAY use enums, tagged unions, or equivalent native types.

### 3.1 Parsed pairing offer

```text
PairingOffer {
  version,
  pairing_id,
  machine_name,
  host,
  port,
  username,
  expected_host_key_fingerprint,
  expires_at_unix,
  temporary_credential
}
```

`temporary_credential` is a secret-bearing in-memory value. Debugging, analytics, crash reports, and user-visible errors MUST redact it. Parsing an offer does not trust a host and does not authorize a connection.

### 3.2 Host-key comparison

Before authentication, the SSH transport obtains the fingerprint of the host key actually presented by `host:port` and returns:

```text
HostKeyComparison {
  expected_fingerprint,
  presented_fingerprint,
  matches
}
```

The app MAY show both fingerprints for comparison, but MUST NOT continue when `matches` is false. When they match, the app MUST still wait for an explicit user trust decision. Cancellation is not trust. Only an affirmative decision permits authentication and pinning of the presented fingerprint.

### 3.3 Pairing request

After explicit trust, the app performs:

```text
InstallPermanentKeyRequest {
  host,
  port,
  username,
  pinned_host_key_fingerprint,
  temporary_credential,
  device_public_key
}
```

`device_public_key` MUST be one single-line OpenSSH `ssh-ed25519` public key. It MUST contain no authorization options, CR/LF injection, or private-key material. The app supplies the UTF-8 public-key line on SSH channel standard input, optionally followed by one LF, then sends EOF. It MUST NOT put the key in a shell command, command-line argument, environment variable, URL, or log.

The app MUST authenticate with `temporary_credential` only after host-key verification and trust. It MUST request no PTY and MUST NOT rely on a requested remote command: the server-side forced command owns the operation.

## 4. Temporary authorization and forced command

The CLI adds one tagged temporary `authorized_keys` line for `pairing_id`. It MUST include a forced provisioning command and restrictions equivalent to:

```text
command="<helper> <pairing_id>",restrict,no-port-forwarding,no-agent-forwarding,no-X11-forwarding,no-pty <temporary-ed25519-public-key> timu-pair:<pairing_id>
```

The helper path is local implementation detail and MUST never cross the app-facing boundary or appear in user-visible errors.

When OpenSSH authenticates the temporary key, the forced command MUST run regardless of any client-requested command and MUST ignore `SSH_ORIGINAL_COMMAND`. It MUST NOT invoke an interactive shell or interpret stdin as shell syntax. It MUST:

1. check the server's current time against the ceremony expiry before reading or mutating authorization;
2. read at most 8,192 bytes from stdin and reject oversized input;
3. trim only surrounding whitespace needed to accept an optional final LF, then validate exactly one single-line `ssh-ed25519` public key;
4. locate exactly one line whose final whitespace-delimited token is `timu-pair:<pairing_id>`;
5. reject a missing or duplicate matching temporary line without mutation;
6. replace that line with the submitted permanent public-key line while preserving unrelated lines;
7. write the complete result to a same-filesystem temporary file using the existing `authorized_keys` mode, then atomically rename it over `authorized_keys`;
8. create the local completion marker only after the atomic replacement succeeds; and
9. exit zero only after both the authorization replacement and completion marker succeed.

Validation, expiry, input-read, or authorization-write failure MUST leave the prior `authorized_keys` content unchanged. If completion-marker creation fails after the atomic authorization commit, the permanent key remains installed and the temporary key remains consumed; the operation reports failure and permanent-key reconnect is the only safe way to determine whether the handoff took effect. The permanent line is the submitted public key as-is after validation; its optional comment is device-supplied metadata, not an authorization or security identifier.

## 5. Atomic handoff, success marker, and CLI completion

The authorization-file replacement is the security handoff: once the rename succeeds, the temporary key is absent and the permanent key is present in the same committed file state. There MUST NOT be a committed state in which both the matching temporary entry and newly submitted permanent entry remain authorized.

The V1 CLI uses a permission-restricted, ceremony-local completion file containing exactly:

```text
paired\n
```

This marker is internal synchronization between the forced command and the waiting CLI; it is not sent to the app and its filesystem location is not protocol data. The waiting CLI treats marker existence as completion, prints its user-facing success message, removes its temporary key files and helper artifacts, and exits successfully.

For the app, SSH channel exit status zero is only provisional handoff success. The app MUST discard the temporary credential and reconnect using the permanent key before returning final success.

## 6. Expiry and replay rejection

Both sides enforce expiry:

- The app rejects the QR when `now >= expires_at_unix` and MUST NOT attempt temporary authentication.
- The forced command rejects when server time is at or later than the expiry passed into the ceremony helper and MUST NOT mutate authorization.
- The CLI stops waiting at expiry and removes its tagged temporary authorization and temporary files.

Successful replacement consumes the credential by removing the only matching temporary authorization. Any second installation attempt therefore fails authentication or, under a direct/internal replay test, fails because no matching `pairing_id` entry exists. It MUST NOT install or replace another permanent key. Duplicate matching temporary entries also fail closed rather than choosing one.

Interruption, cancellation, timeout, and setup failure trigger tagged cleanup. Cleanup MUST remove only this ceremony's temporary line and temporary artifacts; it MUST preserve unrelated authorization entries. Cleanup after successful handoff MUST NOT remove the permanent device key.

## 7. Permanent-key reconnect and final result

After provisional handoff success, the app opens a new SSH connection to the same `host`, `port`, and `username` using the device permanent private key. It MUST compare the presented host key with the pinned fingerprint again. It MUST NOT fall back to the temporary credential or silently start another pairing ceremony.

Only successful host-key verification plus permanent-key authentication yields:

```text
PairingResult.Success {
  machine_name,
  host,
  port,
  username,
  pinned_host_key_fingerprint,
  auth_method = "device_key"
}
```

The result MUST NOT contain either private key, the submitted public-key payload, temporary local paths, helper paths, raw SSH output, or the QR payload. The saved machine profile contains connection metadata and the authentication method kind; permanent private-key material remains in platform secure storage. Readiness starts only after this final success.

## 8. Stable user-corrective outcomes

Error `code` values below are stable app-facing outcomes. User-visible `message` text may evolve, but each code maps to one distinct corrective action. An error MAY carry only nonsensitive metadata explicitly listed here. It MUST NOT include payload/private-key content, the submitted public key, raw command or SSH stderr, usernames embedded in paths, or sensitive local paths.

```text
PairingResult.Failure {
  code,
  message,
  retryable,
  metadata?
}
```

| Code | When used | Corrective action | Retryable | Allowed metadata |
| --- | --- | --- | --- | --- |
| `PAIRING_QR_INVALID` | Envelope, encoding, JSON, required field, port, or key serialization is malformed. | Scan a newly generated QR. | yes | none |
| `PAIRING_VERSION_UNSUPPORTED` | `version` is not supported. | Update timu app/CLI, then generate and scan again. | after update | `received_version` only |
| `PAIRING_EXPIRED` | App or host determines that the five-minute ceremony expired. | Return to the CLI, generate a new QR, and rescan. | yes | `expires_at_unix` only |
| `PAIRING_HOST_UNREACHABLE` | DNS, routing, TCP connection, or SSH handshake cannot reach the selected endpoint. | Check that phone and machine can reach the shown host/port, or rerun CLI with the correct host. | yes | `host`, `port` |
| `PAIRING_HOST_KEY_MISMATCH` | Presented SSH host fingerprint differs from the QR/pinned fingerprint at either connection. | Stop; verify the machine/endpoint and generate a new QR on the intended machine. Never offer bypass. | no, until verified | expected and presented fingerprints |
| `PAIRING_TRUST_DECLINED` | User declines or cancels explicit host trust. | Resume only by reviewing and explicitly trusting the matching fingerprint. | yes | expected fingerprint |
| `PAIRING_TEMP_AUTH_REJECTED` | Host key matched, but temporary SSH public-key authentication failed before provisioning ran. | Generate and scan a new QR; confirm the SSH account/endpoint is unchanged. | yes | none |
| `PAIRING_DEVICE_KEY_INVALID` | Submitted permanent public key is oversized, multiline, unsupported, or malformed. | Regenerate a device Ed25519 keypair and retry with a new QR. | yes | none |
| `PAIRING_ALREADY_USED` | Provisioning finds no unique matching temporary authorization, including a consumed/replayed ceremony. | Do not retry that QR; generate and scan a new one. | yes, with new QR | none |
| `PAIRING_INSTALL_FAILED` | Valid provisioning input cannot be committed or marked complete. | Check SSH authorization-file ownership/permissions on the machine, then generate a new QR. | yes | none |
| `PAIRING_PERMANENT_AUTH_REJECTED` | Handoff appeared successful, but reconnect with the permanent key is rejected. | Keep the generated key in secure storage, verify SSH account authorization on the machine, then start a fresh pairing if needed. | yes | none |
| `PAIRING_CANCELLED` | Local app cancellation before committed handoff. | Start pairing again when ready. | yes | none |

A lower-level implementation error must be mapped to the outcome whose corrective action applies. If safe classification is impossible, use `PAIRING_INSTALL_FAILED` only for failures after provisioning starts; otherwise use the nearest pre-provisioning outcome. Do not create separate public codes merely for different library exceptions that require the same user action.

## 9. V1 conformance sequence

1. Decode and validate the unexpired V1 offer.
2. Connect far enough to obtain the presented host key; compare it exactly with the expected fingerprint.
3. Require explicit user trust and pin the matching fingerprint.
4. Generate/store the device Ed25519 keypair on-device.
5. Authenticate with the temporary key and submit only the device public key over stdin to the forced command.
6. Forced command validates expiry/input, atomically replaces the unique tagged temporary line, writes `paired\n`, and exits zero.
7. CLI observes completion, reports success, and cleans temporary artifacts.
8. App discards temporary authentication material.
9. App reconnects with the permanent private key and the pinned host key.
10. App returns `PairingResult.Success`; only then may readiness begin.

## 10. Current implementation alignment

The current `timu-pair` implementation establishes the V1 QR field names and encoding, opaque random pairing identifiers, five-minute expiry value, shared `is_expired(now >= expires_at_unix)` enforcement in QR decoding and the forced command, restricted shell-quoted forced-command entry, 8,192-byte stdin bound, Ed25519 public-key validation, line-targeted atomic replacement, `paired\n` local marker, and single-use behavior. Its forced-command filesystem read, write, and completion-marker failures are mapped to nonsensitive static error strings. It rejects symlinked `.ssh` directories and `authorized_keys` files before mutation, and its cleanup guard is installed before pairing artifacts are created so every subsequent exit path performs tagged cleanup without removing the installed permanent key.

The following V1 requirements are app/client integration contracts rather than behavior implemented in `timu-pair`: canonical payload validation before connection, exact host-key comparison and explicit trust, stable app-facing result/error mapping, secure device-key generation/storage, interpreting the remote exit status, and permanent-key reconnect. The current forced command reports malformed, missing, duplicate, and replayed cases through one generic validation error; app-facing code MUST map by protocol phase without exposing its raw error text.

Authorization mutations are serialized with an adjacent `0600` lock file; each operation revalidates the directory and `authorized_keys` path after acquiring the lock and atomically renames a same-directory replacement file that preserves the existing `authorized_keys` mode. This implementation and its automated coverage do not replace the required physical-iPhone, macOS Remote Login, and VPS acceptance work.
