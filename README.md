# timu

An open-source mobile app for running persistent AI coding sessions on your own SSH-accessible machine via tmux. Built as a chat-first Expo / React Native frontend backed by a Rust core over FFI.

---

## What It Does

timu lets you connect your phone to a VPS, local Mac, or any SSH-accessible machine and drive coding agents such as Codex, Claude Code, and OpenCode through a WhatsApp-style chat UI. The actual session runs inside a tmux pane on your machine, so it survives app restarts, network drops, and switching devices.

V0 covers:

- Save SSH machine profiles (host, user, port, auth method kind)
- Test the SSH connection with clear, actionable errors
- Verify machine readiness (`tmux`, `git`, Node package managers, agent CLIs)
- Pick a project folder and start a tmux-backed agent session
- Chat with the agent while raw terminal output stays available as a fallback
- Resume the same session after closing or losing the app

---

## Tech Stack

- **Language:** Rust 2024 (core library), TypeScript (mobile app)
- **Frontend:** Expo / React Native with Expo Router
- **Core:** Rust library crate (`timu-core`) exposed to RN via FFI (UniFFI planned)
- **Pairing:** `timu-pair` native CLI, distributed through `npx timu-app`
- **Data:** SQLite via `rusqlite` (Rust-owned; no cloud sync in V0)
- **Transport:** SSH (`russh`), SFTP (`russh-sftp`), tmux on the user's own machine
- **Hosting:** App Store / Play Store target; no hosted backend

---

## Getting Started

### Rust core

```bash
cd timu-core
cargo test
cargo build
```

### Pairing CLI

```bash
cd timu-pair
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

### npm launcher

```bash
cd timu-npx
npm install
npm test
```

### Expo app

```bash
cd timu-app
npm install
npm run dev
```

**Requirements:**

- Rust stable ≥ 1.96
- Node.js 20+ and npm
- An SSH-accessible machine with tmux installed
- One of Codex, Claude Code, or OpenCode on the host for agent sessions

No `.env` or cloud backend is required for V0.

---

## Project Structure

Core modules:

- `timu-core/` — Rust library: SSH transport, host-key TOFU pinning, machine readiness, folder listing, SQLite persistence, tmux session engine
- `timu-app/` — Expo / React Native UI (chat-first, drives timu-core over FFI)
- `timu-pair/` — Native pairing CLI that runs the one-time QR-based onboarding ceremony
- `timu-npx/` — `npx timu-app` launcher: downloads the platform binary and verifies release-artifact checksums before running it

Documentation:

- `AGENTS.md` — Master project context and hard-block security rules
- `docs/prds/v0-prd.md` — V0 product requirements
- `docs/features/` — Feature-specific documentation
- `docs/infra/` — Architecture decisions, schema, testing philosophy

---

## Documentation

Full project documentation lives in:

- [`AGENTS.md`](AGENTS.md) — Master context: identity, security rules, tech stack, conventions
- [`docs/prds/v0-prd.md`](docs/prds/v0-prd.md) — V0 product requirements and user flows
- [`docs/features/`](docs/features/) — Feature-specific docs (timu-core, onboarding CLI, etc.)
- [`docs/infra/`](docs/infra/) — Architecture decisions, schema, API contracts, deployment, testing

---

## V0 User Flow

```txt
Open app
→ Connect a machine
→ Choose auth method
→ Test SSH connection
→ Save machine
→ Check readiness
→ Fix tmux if missing
→ Pick folder
→ Pick agent
→ Start tmux session
→ Chat with coding agent
```

### V0 Success Criteria

A user can:

1. connect their SSH machine from mobile
2. save the connection
3. detect whether `tmux` and agents are installed
4. choose a project folder
5. start a tmux-backed agent session
6. close the app
7. reopen the app
8. resume the same session
9. send messages through a simple chat UI

---

## Security

- SSH secrets (passwords, private keys, passphrases) never live in the persisted `MachineProfile`; they stay in platform secure storage and are supplied at connect time.
- Host keys are verified with trust-on-first-use (TOFU): the fingerprint is shown to the user on first connect and pinned for every subsequent connect.
- The one-time `npx timu-app` pairing credential is short-lived, restricted to a single forced operation, and cleaned up on success, timeout, interruption, or failure.
- V0 has no analytics, telemetry, cloud accounts, or hosted backend.

See `AGENTS.md` §2 for the full hard-block security rules.

---

## Testing

- `cargo test` inside `timu-core/` runs unit and integration tests.
- `cargo test` inside `timu-pair/` runs the pairing CLI integration suite (26+ tests covering QR validation, authorized-keys handling, symlink safety, cleanup, concurrency, and fixture schemas).
- `npm test` inside `timu-npx/` tests launcher behavior and release-artifact checksum verification.

Testing philosophy: `docs/infra/testing.md`.

---

## License

[License TBD]
