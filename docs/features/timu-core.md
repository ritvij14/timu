# timu-core (Rust engine)

> The secure transport + session engine behind the timu mobile app.
> See AGENTS.md for project-wide rules; `docs/prds/v0-prd.md` for V0 scope.

---

## 1. What this owns

timu-core is the Rust library that the Expo app (`timu-app`) drives over FFI
(UniFFI planned). It owns everything the RN layer cannot do reliably on mobile:

- SSH connection, auth, and host-key verification (russh — planned)
- Machine readiness probing (PRD §7/§8) — **landed**
- SFTP folder browsing (PRD §9) — planned
- tmux session lifecycle + streaming (PRD §11/§12) — planned
- Local persistent state for profiles, sessions, recent/favorite folders (PRD §13) — planned
- Secure credential storage bridge (PRD §14) — planned

The RN layer owns UX only. **Rule: tmux is the backend, chat is the frontend;
Rust is the wire between them.**

---

## 2. File ownership

| File | Owns | Status |
| --- | --- | --- |
| `src/lib.rs` | module wiring + public re-exports | landed |
| `src/timu_core.rs` | `TimuCore` facade (holds TOFU pins, `test_connection`) | landed |
| `src/error.rs` | `TimuError` enum + `code()`/`Display`/`user_label` | landed |
| `src/profile.rs` | `MachineProfile`, `AuthMethod`, `ProfileInvalid` | landed |
| `src/readiness.rs` | `Tool`, `ToolStatus`, `ReadinessReport` | landed |
| `src/readiness_probe.rs` | `build_probe_command`, `parse_probe_output` | landed |
| `src/ssh.rs` | `SshTransport` trait, `CommandOutput`, `FakeSshTransport` | landed |
| `src/ssh_russh.rs` | real `russh` transport + TOFU host keys + auth | landed (live test `#[ignore]`) |
| `src/credentials.rs` | `Credentials` carrier (redacting Debug, never serialized) | landed |
| `src/host_key.rs` | `Fingerprint`, `HostKeyPins`, `HostKeyVerdict` (TOFU) | landed |
| `src/connection.rs` | `ConnectionTestResult` + `test_connection` (PRD §6) | landed |
| `src/folder.rs` | `FolderEntry`, shell-based folder listing + `shell_quote` | landed |
| `src/store.rs` | SQLite store: profiles, sessions, recent/favorites, host-key pins | landed |
| `src/secrets.rs` | platform secure-storage bridge | planned |
| `src/tmux.rs` | tmux session engine + streaming | planned |

---

## 3. Security rules

1. **No secrets in `MachineProfile`/`AuthMethod`.** Only the method kind is
   stored. Passwords, key bytes, passphrases live in platform secure storage and
   are passed in via a separate `Credentials` carrier at connect time.
   Enforced by `profile_does_not_carry_secrets` test. (Hard Block §2.1, ADR-002)
2. **No auto-accepting SSH host keys.** First connect = trust-on-first-use with
   the fingerprint surfaced to the user; subsequent connects pin and compare.
   Mismatch = hard failure. (Hard Block §2.2)
3. **No analytics/telemetry in V0.** (PRD §14/§15) A stub layer may exist but
   must do nothing.

---

## 4. Data model

- `MachineProfile { name, host, username, port: u16, auth_method: AuthMethod }`
  — persistable, no secrets. Default port 22.
- `AuthMethod` — `Password | KeyPaste | KeyFile` (kind only).
- `Tool` — `Tmux | Git | Shell | Node | Npm | Pnpm | Yarn | Bun | Codex | Claude | OpenCode`
  (canonical render order).
- `ToolStatus` — `Ready | Missing | Unknown`.
- `ReadinessReport` — map of `Tool → ToolStatus`; `Unknown` default; `tmux_is_missing()`
  predicate drives the PRD §8 install-command path.
- `CommandOutput { stdout, stderr, exit_code: i32 }`.
- `TimuError` — see `src/error.rs`; `code()` is the FFI contract.

---

## 5. Key flows

**Readiness check** (landed):
`build_probe_command()` → `SshTransport::run_command()` → `parse_probe_output()` →
`ReadinessReport::tmux_is_missing()` / `render()`. Fully testable via
`FakeSshTransport`.

**Connection test** (planned, task 8):
`TimuCore::test_connection(profile)` → open SSH → TOFU host key → auth → typed
`ConnectionTestResult` mapping each `TimuError` variant to a PRD §6 failure state.

---

## 6. Testing

- **Runner:** `cargo test` from `timu-core/`.
- **Style:** inline `#[cfg(test)] mod tests` per module + `tests/` for
  cross-crate integration. TDD mandatory (ADR-006).
- **Boundary mock:** `FakeSshTransport` is the only SSH mock. Never mock domain
  types or the store.
- **Currently covered (88 tests, 1 `#[ignore]` live):** error codes/labels,
  profile validation + serde + no-secrets, readiness render/order + tmux
  predicate + serde, probe command + parser edge cases, ssh trait + fake +
  readiness end-to-end, credentials redaction, host-key TOFU
  (first-seen/matches/mismatch + per-host + persistence round-trip), russh
  key-loading path + connect-error classification, connection-result mapping
  for every `TimuError` variant, folder listing + `shell_quote` injection
  guard + missing-dir error, store CRUD + sessions + recent/favorites +
  host-key pins + schema-level no-secret-columns assertion.
- **Explicitly not tested by CI:** live SSH connect against a real sshd
  (`ssh_russh::live_connect_and_run_command`, `#[ignore]`; set
  `TIMU_TEST_SSH_HOST`/`_USER`/`_PASS` or `_KEY`/`_KEYPASS` to run).

---

## 7. Dependencies

- `serde` (derive) — serialization for profiles/reports/folders (forced by round-trip tests).
- `russh` 0.62 — pure-Rust SSH2 client (mobile-friendly, no libssh link).
- `rusqlite` 0.32 (bundled) — SQLite store; `bundled` statically links sqlite3 for mobile cross-compile.
- `tokio` — async runtime for russh + `#[tokio::test]`.
- `serde_json` (dev) — round-trip assertions.
- Planned: `russh-sftp` (only if we move folder listing off the shell command),
  `UniFFI` (FFI to Expo).

New crates are added only when a test forces them (ADR-006).