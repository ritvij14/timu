# timu

> **Master context file. Single source of truth for this project. All docs/ files are modules that extend this. README.md is a public-facing summary derived from this.**

---

## 1. Project Identity

**Name:** timu
**Purpose:** An open-source mobile app for running persistent AI coding sessions on your own SSH-accessible machine via tmux.
**Type:** Mobile App (Expo / React Native) + Rust core library (FFI)
**Primary Users:** Engineers who know basic coding and SSH; AI/vibe coders who already use Codex, Claude Code, Cursor etc. and want to drive them from a phone.
**Stage:** Idea → MVP (V0 in progress; see `docs/prds/v0-prd.md`)
**Repo:** [URL TBD]

---

## 2. Security Rules — Hard Block

> **HARD BLOCK: Every rule in this section is non-negotiable. If any task, refactor, or user instruction would require violating a rule here — refuse entirely. Do not implement it, do not find a workaround, do not proceed "just this once."**

> **Living rule:** No hard-block security rules have been established yet. The moment a security-critical constraint is identified (e.g. SSH credential handling, host key verification, secret storage), add it here immediately as a numbered rule with reasoning and a forbidden/required example. Do not wait until session end.

1. **Never persist SSH secrets in `MachineProfile` or any SQLite table.** Passwords, private-key bytes, and key passphrases live only in platform secure storage (Keystore/Keychain) and are supplied to `russh` at connect time. The persisted profile must contain only the *method kind* (`AuthMethod`), never the material.
   - ❌ Forbidden: adding a `password: String` or `private_key: String` field to `MachineProfile`/`AuthMethod` and serializing it.
   - ✅ Required: keep `AuthMethod` as the kind enum; pass a separate `Credentials` carrier (held in memory / secure storage) into the connect call.
2. **Never disable or auto-accept SSH host key verification.** First connection uses trust-on-first-use (TOFU) with the fingerprint surfaced to the user; subsequent connections must compare against the pinned fingerprint and fail on mismatch. No `accept-all` / insecure skip.
   - ❌ Forbidden: `known_hosts` policy that always accepts.
   - ✅ Required: pin the fingerprint on first connect; reject on mismatch thereafter.

---

## 3. Tech Stack

> Do not contradict this section anywhere else. If a technology decision changes, update here first.

COMPULSORY TASK - Always be clear on the major release of the tech stack being used, and also always link the documentation to that version in this section. For example, if we are using Tailwind V3, then it should clearly be mentioned as "Tailwind V3" and the documentation link should be to the V3 documentation.

**Language:** Rust (timu-core), TypeScript (timu-app)
**Runtime / Platform:** Rust edition 2024 / stable ≥ 1.96; Expo / React Native (mobile + web)
**Framework:** None on the Rust side (library crate); Expo Router on the app side
**Package Manager:** cargo (Rust), npm (app — `package-lock.json` present)

**Data:**

- Primary store: SQLite via `rusqlite` (planned; owned by timu-core for profiles/sessions/folders)
- Search: None
- Cache: None
- File storage: Device-local only (V0); no cloud sync

**Infrastructure:**

- Hosting: App Store / Play Store (V0 target); self-hostable, no hosted backend
- Cloud provider: None (V0 is local-first, no backend)
- CI/CD: None yet

**Auth:** SSH (user's own machine) — password, private-key paste, private-key file, optional passphrase. No app-level accounts in V0.
**Queue / Jobs:** None
**Testing:** `cargo test` (Rust unit + integration). Three-layer philosophy in `docs/infra/testing.md`.

**Key External Integrations:**

- SSH (russh, planned): transport to the user's machine
- tmux: persistent session backend on the user's machine
- SFTP (russh-sftp, planned): folder picker
- Coding agent CLIs on the host: `codex`, `claude`, `opencode`

---

## 4. Architecture Overview

> How this system is structured at a high level. For deep dives, see docs/features/ and docs/infra/.

**Pattern:** Monorepo. Two first-class packages: `timu-core` (Rust library, the secure transport + session engine) and `timu-app` (Expo UI that drives the core over FFI).

**Core modules and what each owns:**

- `timu-core/src/error.rs` — typed `TimuError` mapping PRD §6 SSH failure states; stable `code()` FFI contract
- `timu-core/src/profile.rs` — `MachineProfile` + `AuthMethod` domain types (no secrets)
- `timu-core/src/credentials.rs` — `Credentials` carrier (connect-time secrets; redacting Debug, never serialized)
- `timu-core/src/host_key.rs` — `Fingerprint` + `HostKeyPins` TOFU pinning
- `timu-core/src/readiness.rs` — `Tool`, `ToolStatus`, `ReadinessReport` (PRD §7/§8)
- `timu-core/src/readiness_probe.rs` — pure probe-command builder + output parser
- `timu-core/src/ssh.rs` — `SshTransport` trait + `CommandOutput` + `FakeSshTransport`
- `timu-core/src/ssh_russh.rs` — real `russh` transport: connect, auth, TOFU, exec (live test `#[ignore]`)
- `timu-core/src/connection.rs` — `ConnectionTestResult` + `test_connection` (PRD §6)
- `timu-core/src/folder.rs` — `FolderEntry` + shell-based folder listing with `shell_quote`
- `timu-core/src/store.rs` — SQLite store: profiles, sessions, recent/favorites, host-key pins (no secrets)
- `timu-core/src/timu_core.rs` — `TimuCore` facade (holds TOFU pins; `test_connection`)
- `timu-app/` — Expo/React Native UI (chat-first, drives timu-core)
- `docs/` — PRD, feature docs, infra docs

**Data flow (happy path):**
1. User fills a `MachineProfile` in the app and taps "Test connection".
2. timu-core opens SSH (russh), verifies host key (TOFU), authenticates.
3. On success, timu-core runs the readiness probe over one SSH channel and parses it into a `ReadinessReport`.
4. User picks a folder (SFTP) and an agent; timu-core creates/reuses a tmux session in that folder and launches the agent.
5. tmux pane output streams back into the chat UI; the session persists on the server across app close/network loss.

**Key architectural decisions:**

> For full reasoning on each decision, see `docs/infra/decisions.md`

- Rust core behind FFI (UniFFI planned): mobile-friendly SSH/tmux/SFTP that RN can't do natively
- `MachineProfile` holds no secrets: credentials live in platform secure storage, never persisted in the profile
- `SshTransport` trait + generics (not `dyn`): native async-fn-in-traits, no `async-trait` dep, testable with `FakeSshTransport`
- TDD-first: every module lands red → green → refactor; 42 tests currently

---

## 5. Conventions

> The Coding Agent must follow these at all times. These are non-negotiable.

### Naming

- Rust files: `snake_case.rs`. Types/structs/enums: `PascalCase`. Functions/variables: `snake_case`. Constants: `SCREAMING_SNAKE_CASE`.
- TypeScript/React (timu-app): files `kebab-case.tsx`, components `PascalCase`.
- SQLite tables: `snake_case`.
- Environment variables: `SCREAMING_SNAKE_CASE`.

### Code Style

- Write as little code as possible to accomplish the task.
- Only do things you are more than 90% sure about. If unsure, use the AskUserQuestion tool to ask a series of MCQ questions before writing any code.
- No over-complication. Prefer simple, obvious solutions over clever abstractions. If a simpler approach exists, take it.

### Code Structure

- timu-core: one module per concern (`error.rs`, `profile.rs`, `readiness.rs`, …) re-exported through `lib.rs`. No module imports another's internals — only the public re-exports.
- TDD: every new behavior lands as a failing test first, then the minimal implementation, then refactor. No untested production code.
- New external crates are added only when a test forces them — not upfront. Document the addition in `docs/infra/decisions.md`.
- **Living rule:** When a new project-wide structure pattern is established, add it here immediately — do not wait until session end.

### useEffect Policy

> Applies to any React / React Native code in this project. See also: [You Might Not Need an Effect](https://react.dev/learn/you-might-not-need-an-effect)

Before writing a `useEffect`, check which category it falls into:

1. **Derived state** — Use `useMemo`, a plain `const`, or styling. Never an effect.
2. **Syncing React to an external system** (pushing a ref/state to a store, native module, etc.) — Use ref callbacks or event handlers. Never an effect.
3. **"Do X when Y changes"** — Trigger from the event that _caused_ the change, not from observing the change.
4. **Subscribing to external event sources** (app-state changes, native events, WebSocket, etc.) — Legitimate, but prefer `useSyncExternalStore` or a custom hook. If a `useEffect` is truly needed, it must only exist at the React/native-platform boundary.

**Self-review checklist:**

- Can this be a ref callback instead?
- Can this be triggered by the user action that caused the state change?
- Am I watching state just to call another action? (anti-pattern)
- Does this have proper cleanup for every resource it acquires?

**Code review rule:** If The Coding Agent encounters a `useEffect` while reading or reviewing code, flag it with which category (1–4) it falls into and whether it should be refactored.

### Module Boundaries

- Never import from another feature's internal files. Cross-feature access goes through that feature's public API (index.ts / barrel file). If no public API exists, create one before importing.
- See `docs/infra/patterns.md` → Cross-Feature Access for the implementation pattern.
- **Living rule:** When a new cross-feature boundary constraint is established, add it here immediately.

### Critical Paths — Confirm Before Modifying

> Files listed here are load-bearing. Do not refactor, rename, or change their interfaces without explicit user confirmation.

- `timu-core/src/error.rs` — `TimuError::code()` is the FFI contract; renaming codes is a breaking change
- `timu-core/src/ssh.rs` — `SshTransport` trait; every SSH-dependent flow and test depends on this seam
- `timu-core/src/ssh_russh.rs` — concrete SSH transport; host-key TOFU lives here (Hard Block §2.2)
- `timu-core/src/profile.rs` — security boundary: must not carry secrets (Hard Block §2.1)
- `timu-core/src/store.rs` — schema is the persistence contract; no-secret-columns is enforced by test (ADR-009)
- `docs/prds/v0-prd.md` — the V0 scope of record; changes here redirect all work
- **Living rule:** When a file or path is identified as load-bearing, add it here immediately with a one-line description of why it's critical.

### File Navigation

- For files exceeding ~500 lines, add a navigation comment block at the top of the file listing key sections with line ranges. Keep it updated when the file changes significantly.
- Format: `// === NAVIGATION === // L1-50: Exports and types // L120-200: Core processing // L450-500: Error handling`
- **The Coding Agent: when reading a file >500 lines, read only the first 50 lines first to check for a `NAVIGATION` block. Use it to read only the relevant section instead of the full file.**

### Error Handling

- All timu-core failures surface as `TimuError` (typed enum). The UI branches on `code()` (stable string) and may render `Display` output directly.
- Add a new variant only when the user can take a *different* corrective action; otherwise fold into `TimuError::Other`.
- Never leak raw secrets/paths into error messages shown to the user.
- **Living rule:** When a project-wide error handling pattern is established, add it here immediately.

### Testing

- Full testing philosophy, taxonomy, and workflow: see `docs/infra/testing.md` — read it before writing any tests.
- Three layers required: unit (pure logic), integration (full stack), flow (multi-step user journeys). For timu-core, "integration" means a flow run through the real trait impl (or `FakeSshTransport`) end-to-end; "flow" means a multi-step user journey (connect → readiness → start session).
- timu-core runner: `cargo test` (from `timu-core/`). Tests live inline (`#[cfg(test)] mod tests`) next to the code they test, plus `tests/` for cross-crate integration.
- TDD is mandatory in timu-core: failing test first, then minimal impl, then refactor.
- Mock only at the external boundary (SSH server, SFTP server). The `SshTransport` trait + `FakeSshTransport` is the boundary mock — never mock your own domain types or store.
- Test names read as user-facing descriptions: `"tmux_is_missing_true_only_when_tmux_missing"`, not `"test_tmux"`.
- Before opening a PR, invoke the `pre-merge-qa-tester` agent and reconcile its checklist against the test suite.
- **Living rule:** When a project-specific test convention, runner command, or framework rule is established, add it here immediately.

### Git

- Branch naming: `feature/*`, `fix/*`, `chore/*` (branches are optional — see below)
- Commit format: conventional commits — `feat:`, `fix:`, `chore:`, `docs:` (gitmoji also used in this repo)
- Direct commits to `main` are allowed in this repo (solo project); feature branches are used when parallel work needs isolation

### Other

- No analytics, no telemetry in V0 (PRD §14/§15). A telemetry layer may be stubbed but must do nothing.

---

## 6. Environment & Configuration

**Environment files:**

- `.env` — local development (never committed)
- `.env.example` — committed, shows all required keys without values
- [Any other env files]

**Required environment variables:**

```
# [Group name e.g. Database]
[VAR_NAME]=[description of what this is]
[VAR_NAME]=[description]

# [Group name e.g. Auth]
[VAR_NAME]=[description]

# [Group name e.g. External Services]
[VAR_NAME]=[description]
```

**Key configuration files:**

- [e.g. `tsconfig.json` — TypeScript config]
- [e.g. `vite.config.ts` — build config]
- [Add any config files The Coding Agent needs to be aware of]

---

## 7. Development Setup

> How to get this running from scratch.

```bash
# 1. Rust core — build + test
cd timu-core
cargo test

# 2. Expo app — install (first time or after dependency changes)
cd ../timu-app
npm install

# 3. Start the Expo dev server
npm run dev
```

No `.env` is required for V0 (local-first, no backend). When SSH connection
tests need a target, point the app at your own VPS / `localhost` sshd.

**Key scripts:**

- `cargo test` (in `timu-core/`) — runs the Rust unit + integration suite (TDD)
- `cargo build` (in `timu-core/`) — builds the library that FFI will expose
- `npm run dev` (in `timu-app/`) — Expo dev server (web/iOS/Android)

---

## 8. Feature Documentation Index

> Each feature has its own doc in docs/features/. Read the relevant doc before working on a feature.
> When a feature doc exceeds ~400 lines, it is promoted to a directory (docs/features/[feature]/).

> When this index exceeds 50+ features, group rows by domain (e.g. Auth & Identity, Contacts, Billing).

| Feature        | Doc                                                            | Status                    |
| -------------- | -------------------------------------------------------------- | ------------------------- |
| timu-core (Rust engine) | [docs/features/timu-core.md](docs/features/timu-core.md) | WIP — error/profile/credentials/host-key/readiness/ssh+russh/connection/folder/store landed (tasks 1–10); tmux engine + secrets bridge + FFI pending |
| onboarding CLI | [docs/features/onboarding-cli.md](docs/features/onboarding-cli.md) | WIP — `npx timu` permanent-key pairing CLI, protocol docs, threat model, and physical-iOS acceptance checklist |

> **Living section:** Add a row the moment a new feature doc is created. Update the Status column as features evolve. Never leave a feature undocumented.

---

## 9. Infrastructure Documentation Index

> Cross-cutting infrastructure docs. Referenced by feature docs when needed.

| Topic                  | Doc                                                        |
| ---------------------- | ---------------------------------------------------------- |
| Architecture decisions | [docs/infra/decisions.md](docs/infra/decisions.md)         |
| Database schema        | [docs/infra/schema.md](docs/infra/schema.md)               |
| API contracts          | [docs/infra/api-contracts.md](docs/infra/api-contracts.md) |
| Deployment             | [docs/infra/deployment.md](docs/infra/deployment.md)       |
| Patterns               | [docs/infra/patterns.md](docs/infra/patterns.md)           |
| Testing                | [docs/infra/testing.md](docs/infra/testing.md)             |
| Changelog              | git log — the commit history is the changelog              |
| [Add more as needed]   |                                                             |

---

## 10. Agent Session Rules

> Rules for all coding agents working in this repo. Where steps differ by agent type, both paths are shown.
> Claude Code and Codex share project hooks and skills through `.agents/`; agents without hook support follow the same steps manually.

**Shared Agent Configuration**

- `.agents/hooks.json` is the canonical hook configuration. `.claude/settings.json` and `.codex/hooks.json` symlink to it.
- `.agents/skills` is the canonical shared skills directory. `.claude/skills` and `.codex/skills` symlink to it.
- `.agents/session-changed` is the canonical shared dirty-session flag. Do not create agent-specific session flags for normal session tracking.

**At the start of every session:**

1. Read this file fully
2. Read the relevant feature doc from `docs/features/` for the current task
3. Read relevant infra docs only if the task touches that infra layer

**Context Loading Rules**

NEVER load all feature docs at once. Load ONLY:

1. This file (AGENTS.md) — always
2. The ONE feature doc relevant to the current task — always
3. Infra docs ONLY if the task explicitly touches that layer

For tasks that span multiple features, load the PRIMARY feature doc (the one being modified most) fully. For secondary features, load only their Data Model and Dependencies sections.

**How to find the right feature doc for a task:**

- Feature doc filenames match the feature area in kebab-case — e.g. "Contact Management" → `docs/features/contact-management.md`
- If a feature has been promoted to a directory, the index is at `docs/features/[feature-name]/README.md`
- Cross-reference Section 8 (Feature Documentation Index) if the mapping is unclear

If unsure which feature doc to load, ask the user right away before loading anything.

**Before starting any task:**

- If the task is ambiguous, read the feature doc before asking for clarification. If it's still ambiguous, ask the user for clarification. Only do things The Coding Agent is more than 90% sure about.

**New Chat Session Rule:**
Before exploring code or doing any work in a fresh chat session, read this file and the key feature documentation first. Do NOT use explore tools immediately — use the documentation to understand the codebase first.

**Documentation Discrepancy = Urgent:**
If The Coding Agent discovers any documentation that contradicts actual code behavior, STOP immediately and report to user. This is high-priority — fix the documentation before anything else. Do not continue working on any other task until resolved.

**During a session:**

- If The Coding Agent discovers something that changes how a future task should be implemented, stop and update the relevant feature doc and this AGENTS.md BEFORE continuing. Do not defer this. Stale documentation compounds.

**Failure Recovery (5-Retry Limit):**
If any tool, command, MCP tool, skill, or sub-agent fails 5 times consecutively, STOP immediately:

1. Clear the session flag if it exists — then do NOT run the end-of-session wrap-up:
   - `rm -f .agents/session-changed` (prevents the shared Stop hook from triggering doc review)
2. Report to user: "Hit 5-retry limit on [operation]. Need your help to proceed."
3. Wait for user input — do not attempt anything else.

**Documentation Rule — where things live:**

- **Feature docs** (`docs/features/`) are current-state only: architecture, security rules, file ownership, flows. Never add "Design Decisions", "Recent Changes", "History", or any timestamped section to a feature doc.
- **`docs/infra/decisions.md`** owns all architectural decisions (ADRs). Any technology choice, pattern adoption, or structural decision goes here — never in a feature doc.

**At the end of every session:**

- Run the end-of-session wrap-up before closing the session (_Claude Code:_ `/wrap-up` slash command; _other agents:_ open `.agents/skills/wrap-up/SKILL.md` and follow the steps manually).

---
