# Onboarding CLI Hardening Implementation Plan

> **For Hermes:** Execute this plan phase-by-phase with `delegate_task`; the parent agent remains the sole integrator and verifier. Do not modify `timu-app/`. Do not commit or push unless the user explicitly asks.

**Goal:** Finish the non-visual onboarding work needed for a secure, stable mobile/CLI pairing integration without touching the mobile design implementation.

**Architecture:** Freeze a versioned protocol contract first, harden npm artifact distribution second, then make system interactions testable and security-review filesystem/concurrency behavior. Independent tasks run in parallel only when they have disjoint file ownership; the parent agent reviews every result, resolves conflicts, and runs verification before advancing phases.

**Tech Stack:** Rust 2024 (`timu-pair`), Node.js built-in test runner (`timu-npx`), GitHub Actions, JSON Schema, Markdown.

---

## Orchestration constraints

- Maximum parallel workers: 3.
- Delegated workers inherit the parent model in the current Hermes configuration; `delegate_task` cannot select a cheaper model per call. Cost reduction requires a user/profile-level delegation model configuration. The orchestrator must not claim workers are cheaper unless that configuration is changed.
- Each worker receives exact paths, requirements, security rules, and a prohibition on mobile files.
- A worker may write only its assigned paths.
- The orchestrator reads every changed file and verifies every external side effect.
- Every production behavior follows red → green → refactor.
- Existing behavior tests derive from `PRODUCT-RULES.md`, `docs/prds/v0-prd.md`, and `docs/features/onboarding-cli.md`, not from implementation details.
- No commit, push, release, real `authorized_keys` mutation, `sudo`, or Remote Login change without explicit user approval.

## Phase 0 — Establish a clean coordination baseline

### Step 0.1: Snapshot the worktree
**Owner:** Orchestrator
**Action:** Run `git status --short --branch` and record which files were already modified.
**Verification:** Confirm no mobile path is assigned or touched.

### Step 0.2: Run the current Rust baseline
**Owner:** Orchestrator
**Action:** Run `cargo test` in `timu-pair/`.
**Verification:** Record exact pass/fail counts without modifying code.

### Step 0.3: Run the current Node baseline
**Owner:** Orchestrator
**Action:** Run `npm test` in `timu-npx/`.
**Verification:** Record exact pass/fail counts without modifying code.

### Step 0.4: Create phase ownership boundaries
**Owner:** Orchestrator
**Action:** Assign each phase-1 worker disjoint files.
**Verification:** No two workers may edit the same file in one parallel batch.

---

## Phase 1 — Freeze the mobile/CLI protocol contract

### Step 1.1: Add the canonical QR payload schema test
**Owner:** Worker A
**Files:** Create `timu-pair/tests/fixtures_schema.rs`; create `docs/features/onboarding-cli/pairing-payload-v1.schema.json` only after the test fails.
**Action:** Write one failing Rust test that validates the canonical valid fixture against required V1 fields and field types.
**Verification:** Run `cargo test --test fixtures_schema`; expected initial failure because the schema/fixture is absent.

### Step 1.2: Add the V1 JSON Schema
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/pairing-payload-v1.schema.json`.
**Action:** Define `version`, `pairing_id`, `machine_name`, `host`, `port`, `username`, `host_key_fingerprint`, `expires_at_unix`, and `ephemeral_private_key`; reject unknown properties.
**Verification:** Re-run the focused schema test; expected pass.

### Step 1.3: Add a valid payload fixture
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/fixtures/valid-v1.json`.
**Action:** Add one non-secret synthetic valid V1 payload.
**Verification:** Decode it through `PairingPayload` and assert every field.

### Step 1.4: Add an expired payload fixture
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/fixtures/expired-v1.json`.
**Action:** Add one synthetic expired payload.
**Verification:** Assert decode-at-time rejects it as expired.

### Step 1.5: Add an unsupported-version fixture
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/fixtures/unsupported-v2.json`.
**Action:** Add one syntactically valid payload with version `2`.
**Verification:** Assert it fails with `UnsupportedVersion`.

### Step 1.6: Add a malformed payload fixture
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/fixtures/malformed-v1.json`.
**Action:** Omit one required security field.
**Verification:** Assert decoding fails without logging payload content.

### Step 1.7: Document the permanent-key submission contract
**Owner:** Worker B
**Files:** Create `docs/features/onboarding-cli/pairing-protocol-v1.md`.
**Action:** Specify SSH connection inputs, host-key comparison, explicit trust, stdin public-key format, forced-command behavior, success marker, expiry, replay failure, and reconnect requirement.
**Verification:** Cross-check every statement against `AGENTS.md` hard blocks and `docs/prds/v0-prd.md` §6.

### Step 1.8: Document stable error outcomes
**Owner:** Worker B
**Files:** Modify `docs/features/onboarding-cli/pairing-protocol-v1.md`.
**Action:** Define app-visible outcomes for expired QR, host-key mismatch, authentication failure, malformed device key, replay, timeout, and interrupted CLI.
**Verification:** Ensure no error includes private-key text or sensitive local paths.

### Step 1.9: Review Phase 1 for contract drift
**Owner:** Orchestrator
**Action:** Compare schema, fixtures, Rust fields, and protocol document field-by-field.
**Verification:** Reject the phase if names/types differ across artifacts.

### Step 1.10: Run Phase 1 verification
**Owner:** Orchestrator
**Action:** Run focused fixture tests, full `cargo test`, Clippy, and `git diff --check`.
**Verification:** All pass; report as phase verification, not physical-device acceptance.

---

## Phase 2 — Secure npm release artifacts

### Step 2.1: Write the unsupported-platform failure test
**Owner:** Worker A
**Files:** Create `timu-npx/test/postinstall.test.js`.
**Action:** Add one test asserting unsupported OS/architecture fails installation with an actionable error.
**Verification:** Run `node --test test/postinstall.test.js`; expected failure because current code exits successfully.

### Step 2.2: Make unsupported platforms fail closed
**Owner:** Worker A
**Files:** Modify `timu-npx/postinstall.js`.
**Action:** Return a nonzero exit for unsupported platforms while retaining source-build guidance.
**Verification:** Re-run the focused test; expected pass.

### Step 2.3: Write the HTTP failure test
**Owner:** Worker A
**Files:** Modify `timu-npx/test/postinstall.test.js`.
**Action:** Assert a failed binary download leaves no partial executable and exits nonzero.
**Verification:** Run the focused test; expected initial failure.

### Step 2.4: Make download failures fail closed
**Owner:** Worker A
**Files:** Modify `timu-npx/postinstall.js`.
**Action:** Remove partial files and propagate a nonzero installation failure.
**Verification:** Re-run the focused test; expected pass.

### Step 2.5: Write the checksum mismatch test
**Owner:** Worker A
**Files:** Modify `timu-npx/test/postinstall.test.js`.
**Action:** Assert a downloaded binary whose SHA-256 differs from the release checksum is deleted and never chmodded.
**Verification:** Run the focused test; expected initial failure.

### Step 2.6: Implement streaming SHA-256 verification
**Owner:** Worker A
**Files:** Modify `timu-npx/postinstall.js`.
**Action:** Download the binary and matching `.sha256`, compute with Node `crypto`, compare using exact normalized hex, and delete on mismatch.
**Verification:** Re-run the checksum mismatch test; expected pass.

### Step 2.7: Write the checksum success test
**Owner:** Worker A
**Files:** Modify `timu-npx/test/postinstall.test.js`.
**Action:** Assert a matching artifact becomes executable only after verification.
**Verification:** Focused test passes.

### Step 2.8: Generate checksum assets in release CI
**Owner:** Worker B
**Files:** Modify `.github/workflows/release.yml`.
**Action:** Generate `${asset}.sha256` after rename and upload both files.
**Verification:** Validate YAML and inspect that every matrix artifact gets one checksum.

### Step 2.9: Write the package-install smoke test
**Owner:** Worker C
**Files:** Create `timu-npx/test/package-smoke.test.js`.
**Action:** Pack the npm package, install it in a temporary directory with a controlled downloader seam, and invoke the launcher against a fake native binary.
**Verification:** Test fails before required seams exist, then passes after minimal seam support.

### Step 2.10: Review Phase 2 security properties
**Owner:** Orchestrator
**Action:** Verify no binary executes or receives executable permission before hash verification.
**Verification:** Confirm unsupported platform, network failure, HTTP error, truncation, and mismatch all exit nonzero.

### Step 2.11: Run Phase 2 verification
**Owner:** Orchestrator
**Action:** Run `npm test`, syntax checks, `npm pack --dry-run`, workflow YAML validation, and `git diff --check`.
**Verification:** All pass.

---

## Phase 3 — Make CLI system behavior integration-testable

### Step 3.1: Define command-runner behavior with a failing test
**Owner:** Worker A
**Files:** Create `timu-pair/tests/system_boundaries.rs`; modify `timu-pair/src/lib.rs` only after failure.
**Action:** Add a fake command runner test for SSH-listener detection and Remote Login status.
**Verification:** Focused test initially fails because no injectable boundary exists.

### Step 3.2: Extract the command runner boundary
**Owner:** Worker A
**Files:** Modify `timu-pair/src/lib.rs` and `timu-pair/src/main.rs`.
**Action:** Introduce the smallest trait/struct seam needed to fake `ssh-keygen`, `networksetup`, `ipconfig`, `tailscale`, `hostname`, `sudo`, and time.
**Verification:** Existing CLI behavior remains unchanged and focused test passes.

### Step 3.3: Test explicit Remote Login decline
**Owner:** Worker A
**Files:** Modify `timu-pair/tests/system_boundaries.rs`.
**Action:** Assert a `no` response makes no `sudo` call and returns actionable failure.
**Verification:** Focused test passes.

### Step 3.4: Test Remote Login authorization failure
**Owner:** Worker A
**Files:** Modify `timu-pair/tests/system_boundaries.rs`.
**Action:** Assert a failed OS authorization exits without creating temporary credentials.
**Verification:** Focused test passes.

### Step 3.5: Test single-address auto-selection
**Owner:** Worker B
**Files:** Create `timu-pair/tests/address_selection_flow.rs`.
**Action:** Assert one Wi-Fi/Ethernet/Tailscale candidate requires no prompt.
**Verification:** Focused test passes after minimal extraction.

### Step 3.6: Test multiple-address numbered selection
**Owner:** Worker B
**Files:** Modify `timu-pair/tests/address_selection_flow.rs`.
**Action:** Assert multiple candidates display numbered labels and accept only a valid option number.
**Verification:** Invalid text, zero, and out-of-range values fail without starting pairing.

### Step 3.7: Test macOS address parsing
**Owner:** Worker B
**Files:** Modify `timu-pair/tests/address_selection_flow.rs` and `timu-pair/src/lib.rs`.
**Action:** Feed synthetic `networksetup`/`ipconfig` output and assert Wi-Fi and Ethernet labels remain correct.
**Verification:** No live network state is required.

### Step 3.8: Test Tailscale discovery
**Owner:** Worker B
**Files:** Modify `timu-pair/tests/address_selection_flow.rs`.
**Action:** Feed synthetic `tailscale ip -4` output and assert only valid Tailscale IPv4 candidates appear.
**Verification:** Docker, loopback, and malformed addresses are excluded.

### Step 3.9: Test timeout cleanup
**Owner:** Worker C
**Files:** Create `timu-pair/tests/session_cleanup.rs`.
**Action:** Use fake time and temporary files to assert timeout removes only the tagged temporary key and session directory.
**Verification:** Existing unrelated lines and permissions survive.

### Step 3.10: Test interrupt cleanup
**Owner:** Worker C
**Files:** Modify `timu-pair/tests/session_cleanup.rs`.
**Action:** Trigger the injectable cancellation boundary and assert the same cleanup guarantees.
**Verification:** No real signal or user SSH file is touched.

### Step 3.11: Test startup failure cleanup
**Owner:** Worker C
**Files:** Modify `timu-pair/tests/session_cleanup.rs`.
**Action:** Fail after key generation but before QR output and assert private/public keys and temporary authorization are removed.
**Verification:** Every acquired resource has a cleanup assertion.

### Step 3.12: Review Phase 3 architecture
**Owner:** Orchestrator
**Action:** Ensure boundaries are narrow and production remains KISS; reject generalized framework abstractions.
**Verification:** `main.rs` orchestrates; pure/security logic lives in `lib.rs`; no mobile dependency is introduced.

### Step 3.13: Run Phase 3 verification
**Owner:** Orchestrator
**Action:** Run all Rust tests, Clippy with warnings denied, release build, and `git diff --check`.
**Verification:** All pass.

---

## Phase 4 — Filesystem and concurrency hardening

### Step 4.1: Add a symlinked `authorized_keys` rejection test
**Owner:** Worker A
**Files:** Create `timu-pair/tests/filesystem_security.rs`.
**Action:** Assert pairing refuses a symlink target and does not modify the linked file.
**Verification:** Focused test initially fails if current code follows symlinks.

### Step 4.2: Reject unsafe SSH paths
**Owner:** Worker A
**Files:** Modify `timu-pair/src/lib.rs` and `timu-pair/src/main.rs`.
**Action:** Use non-following metadata checks and fail closed on symlinked `.ssh` or `authorized_keys`.
**Verification:** Symlink test passes.

### Step 4.3: Add ownership and permissions tests
**Owner:** Worker A
**Files:** Modify `timu-pair/tests/filesystem_security.rs`.
**Action:** Assert unsafe owner/mode states are rejected or safely normalized according to documented policy.
**Verification:** Unrelated permissions are not broadened.

### Step 4.4: Add atomic replacement preservation tests
**Owner:** Worker A
**Files:** Modify `timu-pair/tests/filesystem_security.rs`.
**Action:** Assert mode and unrelated content survive successful replacement.
**Verification:** Focused test passes.

### Step 4.5: Add concurrent-session isolation test
**Owner:** Worker B
**Files:** Create `timu-pair/tests/concurrent_pairing.rs`.
**Action:** Create two pairing IDs and assert completing one leaves the other temporary entry untouched.
**Verification:** Focused test passes.

### Step 4.6: Add same-ID collision test
**Owner:** Worker B
**Files:** Modify `timu-pair/tests/concurrent_pairing.rs`.
**Action:** Assert duplicate live pairing IDs fail rather than sharing files or authorization.
**Verification:** Focused test passes.

### Step 4.7: Add serialized `authorized_keys` update test
**Owner:** Worker B
**Files:** Modify `timu-pair/tests/concurrent_pairing.rs` and production code minimally.
**Action:** Assert overlapping updates cannot lose unrelated lines.
**Verification:** Repeat the test enough times to expose race-prone writes without relying on sleeps.

### Step 4.8: Review Phase 4 against hard security rules
**Owner:** Orchestrator
**Action:** Trace every exit path from credential creation through cleanup.
**Verification:** Pairing remains restricted, five-minute, single-use, and line-preserving.

### Step 4.9: Run Phase 4 verification
**Owner:** Orchestrator
**Action:** Run all Rust tests, Clippy, release build, and `git diff --check`.
**Verification:** All pass.

---

## Phase 5 — Threat model and native integration contract

### Step 5.1: Draft the pairing threat model
**Owner:** Worker A
**Files:** Create `docs/features/onboarding-cli/threat-model.md`.
**Action:** Enumerate assets, trust boundaries, adversaries, and mitigations for QR theft, replay, host-key substitution, malicious public-key input, interrupted provisioning, symlink attacks, concurrent pairing, and compromised release artifacts.
**Verification:** Every threat maps to a concrete test, hard rule, or explicit residual risk.

### Step 5.2: Review threat model completeness
**Owner:** Orchestrator
**Action:** Compare threats against implementation and test names.
**Verification:** Add no speculative V1 architecture; document only V0.

### Step 5.3: Define mobile-facing data types without mobile code
**Owner:** Worker B
**Files:** Modify `docs/features/onboarding-cli/pairing-protocol-v1.md`.
**Action:** Specify language-neutral request/result structures for decode, fingerprint confirmation, key submission, completion, and reconnect.
**Verification:** No `timu-app/` file changes.

### Step 5.4: Define stable protocol error codes
**Owner:** Worker B
**Files:** Modify `docs/features/onboarding-cli/pairing-protocol-v1.md`.
**Action:** Add a minimal code table for app branching without binding screen copy.
**Verification:** Each code represents a distinct corrective action.

### Step 5.5: Create the physical-iPhone acceptance checklist
**Owner:** Worker C
**Files:** Create `docs/features/onboarding-cli/physical-ios-acceptance.md`.
**Action:** List Mac prerequisites, command invocation, fingerprint comparison, device-key handoff, reconnect, readiness transition, and cleanup inspection.
**Verification:** Checklist distinguishes automated evidence from manual evidence.

### Step 5.6: Add VPS acceptance variation
**Owner:** Worker C
**Files:** Modify `docs/features/onboarding-cli/physical-ios-acceptance.md`.
**Action:** Add `--host`, nonstandard port, firewall, and public DNS checks without creating a separate protocol.
**Verification:** Mac remains the first path; VPS remains later validation.

### Step 5.7: Run documentation consistency review
**Owner:** Orchestrator
**Action:** Compare `AGENTS.md`, `PRODUCT-RULES.md`, PRD, onboarding feature docs, schema, and fixtures.
**Verification:** Stop and fix any contradiction before proceeding.

---

## Phase 6 — Final integration verification

### Step 6.1: Run Rust formatting verification
**Owner:** Orchestrator
**Action:** Run `cargo fmt --manifest-path timu-pair/Cargo.toml -- --check`.
**Verification:** Exit 0.

### Step 6.2: Run the full Rust test suite
**Owner:** Orchestrator
**Action:** Run `cargo test --manifest-path timu-pair/Cargo.toml`.
**Verification:** Exit 0 with exact test count recorded.

### Step 6.3: Run Rust lint verification
**Owner:** Orchestrator
**Action:** Run `cargo clippy --manifest-path timu-pair/Cargo.toml --all-targets -- -D warnings`.
**Verification:** Exit 0.

### Step 6.4: Build the release binary
**Owner:** Orchestrator
**Action:** Run `cargo build --manifest-path timu-pair/Cargo.toml --release`.
**Verification:** Exit 0 and binary exists.

### Step 6.5: Run all npm tests
**Owner:** Orchestrator
**Action:** Run `npm test` in `timu-npx/`.
**Verification:** Exit 0 with exact test count recorded.

### Step 6.6: Verify JavaScript syntax
**Owner:** Orchestrator
**Action:** Run `node --check bin/timu.js` and `node --check postinstall.js`.
**Verification:** Both exit 0.

### Step 6.7: Verify npm package contents
**Owner:** Orchestrator
**Action:** Run `npm pack --dry-run` in `timu-npx/`.
**Verification:** Only intended runtime files are included.

### Step 6.8: Verify release workflow syntax
**Owner:** Orchestrator
**Action:** Parse `.github/workflows/release.yml` with the repository-available YAML validator.
**Verification:** Exit 0.

### Step 6.9: Run repository whitespace verification
**Owner:** Orchestrator
**Action:** Run `git diff --check`.
**Verification:** Exit 0.

### Step 6.10: Run an independent security review subagent
**Owner:** Review worker
**Action:** Review only the final diff for credential leakage, command injection, symlink/race issues, cleanup gaps, and checksum bypasses.
**Verification:** Orchestrator independently validates every finding before changes.

### Step 6.11: Run an independent requirements review subagent
**Owner:** Review worker
**Action:** Compare final behavior against product rules, PRD, feature contract, and hard blocks.
**Verification:** Resolve all confirmed mismatches.

### Step 6.12: Report remaining manual acceptance
**Owner:** Orchestrator
**Action:** State exactly what remains unverified until the mobile scanner/client is integrated.
**Verification:** Never describe physical pairing as passing before it is performed.

---

## Phase execution policy

1. Dispatch only one phase at a time.
2. Run at most three disjoint workers concurrently.
3. Do not dispatch the next phase until the orchestrator has reviewed diffs and phase verification passes.
4. If a worker edits an unassigned file, stop and inspect before continuing.
5. If tests expose a requirements contradiction, stop implementation and resolve documentation first.
6. If a failure is caused by test isolation or a wrong requirement, fix the test; do not weaken accurate security behavior.
7. If production behavior violates the requirement, keep the test and fix production.
8. Preserve the user’s existing dirty worktree; never reset or overwrite unrelated edits.
9. Keep mobile design and implementation paths untouched throughout this plan.
10. Do not commit or push unless explicitly requested.

## Primary risks

- The current npm installer exits successfully after unsupported-platform and download failures; Phase 2 intentionally changes this to fail closed.
- Current live-system orchestration is concentrated in `main.rs`; Phase 3 must extract only narrow seams and avoid a framework rewrite.
- `authorized_keys` updates need symlink, ownership, mode, and concurrency review before being considered robust.
- The QR necessarily carries a temporary private key; its five-minute lifetime, restricted forced command, non-logging rule, and deterministic cleanup remain critical.
- Per-call cheaper subagent selection is unavailable through the current delegation tool; model cost must be configured outside this plan if required.

## Completion criteria

- Versioned schema, fixtures, and protocol document agree exactly.
- npm release binaries are checksum-verified before chmod/execution.
- Unsupported platforms and failed/corrupt downloads fail nonzero with actionable output.
- Remote Login, address selection, timeout, cancellation, and startup cleanup are tested through fake boundaries.
- Symlink, permissions, atomicity, and concurrent pairing behaviors have requirements-driven tests.
- Threat model and physical acceptance checklist are complete.
- All automated verification commands pass.
- No mobile files were changed.
- Remaining physical-iPhone and VPS checks are reported honestly.