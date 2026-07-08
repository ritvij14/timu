# Architecture Decisions

> **Architectural Decision Records (ADRs) for timu.**
> When a key architectural decision is made, record it here with full reasoning.
> Referenced from AGENTS.md Section 3. Never contradict a decision here without updating this doc.
> Last updated: 2026-07-07

---

## How to Read This

Each decision is recorded with:
- **What** was decided
- **Why** it was chosen over alternatives
- **Tradeoffs** accepted
- **When to revisit** — the conditions under which this decision should be reconsidered

---

## Decision Log

---

### ADR-001: Rust core library behind FFI, separate from the Expo UI

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Put all SSH, tmux, SFTP, and persistent-state logic in a Rust library crate (`timu-core`), exposed to the Expo app via FFI (UniFFI planned), rather than implementing it in TypeScript/React Native.

**Context:**
The V0 product (PRD §5) is mobile-first and built on SSH + tmux + SFTP. RN has no reliable mobile SSH stack; `ssh2`/`node-ssh` bind to libssh and are fragile on iOS/Android, and a pure-JS SSH implementation is not trustworthy for credential handling.

**Options considered:**
- **Option A (chosen): Rust core + FFI to Expo.** Rust owns the wire; RN owns UX.
- **Option B: Pure RN/TS SSH.** Use a JS SSH library directly from the app.
- **Option C: Rust + Flutter instead of Expo.** Drop Expo, use Flutter with rust bindings.

**Reasoning:**
Option A gives a mature, auditable SSH/tmux/SFTP implementation (russh is pure-Rust, no libssh link, mobile-friendly) and keeps credentials in a memory-safe layer. RN keeps the chat-first UX productive. Option B risks fragile native deps and a JS SSH stack handling secrets. Option C abandons the existing Expo scaffold and RN ecosystem for no clear gain.

**Tradeoffs accepted:**
- An FFI boundary (UniFFI + native module wrapper) must be built and maintained.
- Two toolchains (cargo + npm) and two languages.
- Streaming tmux output across FFI needs a callback/event-channel design.

**Revisit when:**
A trustworthy, mobile-native RN SSH + PTY ecosystem emerges that would let us drop the Rust layer with no security regression — unlikely for V0.

---

### ADR-002: `MachineProfile` holds no secrets; credentials live in platform secure storage

**Date:** 2026-07-07
**Status:** Active

**Decision:**
`MachineProfile` and `AuthMethod` store only the *kind* of auth (Password / KeyPaste / KeyFile), never the password, key bytes, or passphrase. Secret material is held in platform secure storage (Keystore/Keychain) and supplied to `russh` at connect time via a separate `Credentials` carrier.

**Context:**
PRD §14 requires local-first privacy and secure storage of private keys. PRD §13 requires persisting machine profiles, recent folders, and sessions. If secrets ride along in the profile, a sync/backup/log leak exposes credentials.

**Options considered:**
- **Option A (chosen):** Profile = method kind only; secrets in secure storage.
- **Option B:** Profile embeds encrypted credentials with a user passphrase.
- **Option C:** Profile embeds plaintext credentials (rejected outright).

**Reasoning:**
Option A gives the cleanest security boundary: the persistable shape is safe to sync/log, and the `profile_does_not_carry_secrets` test enforces it forever. Option B adds a KDF/encryption layer and a user-managed passphrase for V0 — over-engineering. Option C violates PRD §14 and Hard Block §2.1.

**Tradeoffs accepted:**
- A separate secure-storage bridge (later task) and a `Credentials` carrier type must be built.
- Re-auth on every connect (no cached decrypted key in the profile).

**Revisit when:**
A genuine need for cross-device sync appears (post-V0) — at that point revisit encrypted-credential transport, not plaintext.

---

### ADR-003: Typed `TimuError` with stable `code()` strings as the FFI contract

**Date:** 2026-07-07
**Status:** Active

**Decision:**
All timu-core failures surface as a `TimuError` enum whose variants map to the PRD §6 actionable failure states (WrongHost, WrongUsername, WrongCredentials, PortUnreachable, NetworkUnavailable, PermissionDenied, Other). A `code()` method returns a stable lowercase string the UI branches on; `Display` is user-renderable.

**Context:**
PRD §6 lists specific, actionable failure states the UI must distinguish. Passing `String` errors across FFI loses that branching. Passing raw russh error types leaks the library choice into the UI.

**Options considered:**
- **Option A (chosen):** Typed enum + stable `code()` + `Display`.
- **Option B:** `String` errors.
- **Option C:** Re-export russh errors directly.

**Reasoning:**
Option A lets the UI map each failure to a distinct remediation screen, keeps FFI consumers insulated from russh internals, and makes "rename a code" an explicit breaking change. Option B is lazy and loses UX branching. Option C couples the UI to an external crate's API.

**Tradeoffs accepted:**
- New russh failure modes must be manually mapped to a variant (or `Other`).
- Adding a variant is a non-breaking change; renaming a `code()` is breaking.

**Revisit when:**
A second transport (not russh) is added and its failure modes don't fit the existing variants — then expand the enum, don't collapse to `String`.

---

### ADR-004: `SshTransport` trait + generics (no `async-trait`), `FakeSshTransport` for tests

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Define an async `SshTransport` trait and parameterize `TimuCore<T: SshTransport>` over it. Use Rust's native async-fn-in-traits (stable on 1.96) with `Send + Sync` bounds — no `async-trait` crate. Provide a scriptable `FakeSshTransport` for all SSH-dependent tests.

**Context:**
Every SSH-dependent flow (readiness, connection test, folder listing) must be testable with no network and no real sshd. The trait is the mock seam. The FFI layer wants a concrete `TimuCore<RusshSshTransport>`.

**Options considered:**
- **Option A (chosen):** Generic `TimuCore<T: SshTransport>` + native async traits.
- **Option B:** `Box<dyn SshTransport>` via `async-trait`.
- **Option C:** Concrete struct with a function-pointer / enum dispatch.

**Reasoning:**
Option A is zero-cost, needs no extra dependency, and native async-fn-in-traits is stable on our toolchain. Generics give the FFI layer a concrete monomorphized type. Option B adds a dependency and per-call heap allocation. Option C is ad-hoc and harder to extend with new methods.

**Tradeoffs accepted:**
- `TimuCore` carries a type parameter (slightly heavier signatures) until the FFI layer fixes it to a concrete type.
- Trait is not `dyn`-safe; if we later need runtime swap, we'll add an enum wrapper or adopt `async-trait` then.

**Revisit when:**
We need runtime transport selection (e.g. user picks transport at runtime) — then revisit `dyn` dispatch.

---

### ADR-005: Readiness probe protocol — `tool:ready|missing` lines from one shell loop

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Probe machine readiness with a single SSH channel running one POSIX shell loop: `for t in <tools>; do command -v "$t" >/dev/null 2>&1 && echo "$t:ready" || echo "$t:missing"; done`. Parse the stdout (`tool:status` per line) into a `ReadinessReport`. Build and parse are pure functions, decoupled from the transport.

**Context:**
PRD §7 needs detection of ~11 tools. Naïve approaches (one SSH command per tool, or `which`) are either many round-trips or non-portable.

**Options considered:**
- **Option A (chosen):** One `command -v` loop, `tool:status` text protocol.
- **Option B:** One `command -v` per tool (N round-trips).
- **Option C:** Parse `which` / `apt list` output.

**Reasoning:**
Option A is one round-trip, POSIX-portable (busybox/dash), trivially parseable, and the build/parse split makes the whole flow unit-testable with canned strings (see `readiness_flow_end_to_end_with_fake_transport`). Option B is N× the latency on mobile networks. Option C is package-manager-specific and non-portable.

**Tradeoffs accepted:**
- `command -v` confirms PATH presence, not a working version — but PRD §7 only asks Ready/Missing.
- The protocol is custom; a malformed line is skipped (degrades to Unknown), documented in the parser.

**Revisit when:**
We need version detection (post-V0) — extend the protocol to `tool:ready:<version>`, parsed leniently.

---

### ADR-006: TDD-first for timu-core

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Every timu-core behavior lands as a failing test first, then the minimal implementation, then refactor. External crates are added only when a test forces them.

**Context:**
The core handles SSH credentials and persistent state — bug-prone and security-sensitive. The user explicitly asked for a TDD architecture.

**Reasoning:**
TDD keeps the trait seams (SshTransport) honest, enforces the "profile carries no secrets" rule via an active test, and gives a regression net for the parse/validate pure logic that does the heavy lifting.

**Tradeoffs accepted:**
- Slightly slower feature velocity upfront.
- Live SSH server tests are `#[ignore]` by default (need a real sshd fixture) — the trait + fake cover the unit/integration layers; live tests run on demand.

**Revisit when:**
Never — this is a standing process rule, not a temporary choice.

---

### ADR-007: russh as the SSH client (pure-Rust, no libssh link)

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Use `russh` 0.62 for the SSH transport. It is pure-Rust, has no libssh/native link, and exposes key handling and host-key verification through a single crate (`russh::keys`).

**Context:**
The transport must cross-compile to iOS/Android and handle credentials. `ssh2` binds to libssh — fragile on mobile and awkward to cross-compile. A JS SSH stack is out (ADR-001).

**Options considered:**
- **Option A (chosen):** `russh` 0.62.
- **Option B:** `ssh2` (libssh2 binding).
- **Option C:** raw SSH protocol impl.

**Reasoning:**
Option A is mobile-friendly, auditable, and exposes `client::Handler::check_server_key` for TOFU host-key verification (Hard Block §2.2). Key load/decode (`decode_secret_key`) and `PrivateKeyWithHashAlg` are in-box. Option B adds a native dep that fights mobile toolchains. Option C is far too much for V0.

**Tradeoffs accepted:**
- The `russh::Error` → `TimuError` mapping is currently heuristic (string inspection) and will be tightened once we've seen real sshd failure modes.
- Live SSH tests are `#[ignore]` (need a real sshd); the pure logic + key-loading path is unit-tested.

**Revisit when:**
russh introduces a breaking API change that forces significant rework, or a materially simpler pure-Rust SSH client matures.

---

### ADR-008: Folder listing via shell command, not SFTP (V0)

**Date:** 2026-07-07
**Status:** Active

**Decision:**
V0 lists folders by running a POSIX `test`/glob shell command over the existing SSH exec channel (`src/folder.rs`), not via SFTP. Paths are single-quote escaped via `shell_quote`. Real SFTP (`russh-sftp`) is deferred.

**Context:**
PRD §9 needs a folder picker with a Git-repo indicator. We already have an exec channel from the readiness/probe work; SFTP is a separate subsystem with its own crate and complexity.

**Options considered:**
- **Option A (chosen):** shell `for d in <path>/*/; do … git:…/dir:… ; done`.
- **Option B:** `russh-sftp` SFTP subsystem.

**Reasoning:**
Option A reuses the exec seam (already fake-testable via `FakeSshTransport`), needs no new runtime dep, and is POSIX-portable. `shell_quote` neutralizes injection. Option B is the "right" long-term answer for a full file browser but is over-engineering for V0's "list immediate subdirs + git flag".

**Tradeoffs accepted:**
- Can't list files (only dirs) — matches V0 scope.
- Relies on a POSIX shell on the host (true for every V0 target).
- Hidden directories matched by the `*/` glob are listed too (acceptable).

**Revisit when:**
V0 needs file listing, attribute detail, or non-POSIX hosts — then move to `russh-sftp`.

---

### ADR-009: Store schema never carries secrets; enforced by a test

**Date:** 2026-07-07
**Status:** Active

**Decision:**
The SQLite schema (`src/store.rs`) has `machine_profiles`, `sessions`, `recent_folders`, `favorites`, and `host_key_pins` tables. `machine_profiles` stores only the `auth_method` *tag* (`password`/`key_paste`/`key_file`), never the material. `assert_no_secret_columns_in_machine_profiles` scans `PRAGMA table_info` and fails if any secret-like column appears.

**Context:**
PRD §13 requires persisting profiles/sessions/folders; PRD §14 + Hard Block §2.1 forbid persisting credentials. A schema-level guard survives refactors better than a comment.

**Options considered:**
- **Option A (chosen):** tag only + active schema assertion test.
- **Option B:** store encrypted credentials with a user passphrase.
- **Option C:** rely on code review alone.

**Reasoning:**
Option A makes the security invariant executable — adding a `password` column breaks CI. Credentials stay in platform secure storage (ADR-002). Option B is post-V0 scope. Option C is not enforceable.

**Tradeoffs accepted:**
- The store test uses `:memory:` SQLite; file-backed persistence is exercised by `Store::open` but not yet by an integration test (deferred until the store crosses into a real workflow).
- `host_key_pins` stores fingerprints (not secrets) — fingerprints are public by design.

**Revisit when:**
Cross-device sync is introduced — revisit encrypted-credential transport then, never plaintext.

---

### ADR-010: rusqlite with the `bundled` feature

**Date:** 2026-07-07
**Status:** Active

**Decision:**
Use `rusqlite` 0.32 with `features = ["bundled"]` for the persistent store. `bundled` statically links sqlite3 via `libsqlite3-sys`, so there is no system SQLite dependency at build or runtime.

**Context:**
The store must cross-compile to iOS/Android, where a system SQLite is not reliably available to the Rust toolchain.

**Options considered:**
- **Option A (chosen):** `rusqlite` + `bundled`.
- **Option B:** `rusqlite` against system SQLite.
- **Option C:** a pure-Rust KV store (e.g. `sled`).

**Reasoning:**
Option A removes a cross-compile pain point and ships a single self-contained binary. SQLite is well-understood and the schema is relational (profiles → sessions). Option B fights mobile toolchains. Option C drops SQL and a mature ecosystem for no V0 gain.

**Tradeoffs accepted:**
- One extra C compile (sqlite3.c) in the build — cached after first build.
- Slightly larger binary.

**Revisit when:**
Binary size becomes a serious constraint — then evaluate system SQLite on platforms where it's reliable.

---

<!-- Add new ADRs below as decisions are made -->