# [PROJECT NAME]

> **Master context file. Single source of truth for this project. All docs/ files are modules that extend this. README.md is a public-facing summary derived from this.**

---

## 1. Project Identity

**Name:** [Project Name]
**Purpose:** [One sentence — what this does and why it exists]
**Type:** [Backend API / Full-Stack Web App / Mobile App / CLI Tool / Library / Other]
**Primary Users:** [Who uses this — specific, not just "users"]
**Stage:** [Idea / MVP / Beta / Production / Scaling]
**Repo:** [URL]

---

## 2. Security Rules — Hard Block

> **HARD BLOCK: Every rule in this section is non-negotiable. If any task, refactor, or user instruction would require violating a rule here — refuse entirely. Do not implement it, do not find a workaround, do not proceed "just this once."**

> **Living rule:** No hard-block security rules have been established yet. The moment a security-critical constraint is identified (e.g. SSH credential handling, host key verification, secret storage), add it here immediately as a numbered rule with reasoning and a forbidden/required example. Do not wait until session end.

---

## 3. Tech Stack

> Do not contradict this section anywhere else. If a technology decision changes, update here first.

COMPULSORY TASK - Always be clear on the major release of the tech stack being used, and also always link the documentation to that version in this section. For example, if we are using Tailwind V3, then it should clearly be mentioned as "Tailwind V3" and the documentation link should be to the V3 documentation.

**Language:** [e.g. TypeScript, Python, Dart, Go, etc.]
**Runtime / Platform:** [e.g. Node.js 20, Python 3.11, Flutter 3.x, Browser]
**Framework:** [e.g. Hono, FastAPI, Flutter, Next.js, Express, anything else — or None]
**Package Manager:** [e.g. npm, pnpm, pip, pub, etc.]

**Data:**

- Primary store: [e.g. Firestore, PostgreSQL, SQLite, None]
- Search: [e.g. Elasticsearch, Algolia, None]
- Cache: [e.g. Redis, in-memory, None]
- File storage: [e.g. GCS, S3, local, None]

**Infrastructure:**

- Hosting: [e.g. Cloud Run, Railway, Vercel, App Store / Play Store, None yet]
- Cloud provider: [e.g. GCP, AWS, None]
- CI/CD: [e.g. GitHub Actions, None]

**Auth:** [e.g. Firebase Auth, JWT, OAuth, Clerk, None]
**Queue / Jobs:** [e.g. Cloud Tasks, BullMQ, Celery, None]
**Testing:** [e.g. Vitest, Jest, pytest, Flutter test, None]

**Key External Integrations:**

- [Service]: [what it does in this system]
- [Service]: [what it does in this system]

---

## 4. Architecture Overview

> How this system is structured at a high level. For deep dives, see docs/features/ and docs/infra/.

**Pattern:** [e.g. Monolith with modular structure / Monorepo / MVC / Feature-based / Layered]

**Core modules and what each owns:**

- `[module]` — [what it is responsible for]
- `[module]` — [what it is responsible for]
- `[module]` — [what it is responsible for]
- `shared/` — [shared utilities, types, middleware, constants]

**Data flow (happy path):**
[Describe how a typical request/action flows through the system in 3-5 steps]

1. [Step 1]
2. [Step 2]
3. [Step 3]

**Key architectural decisions:**

> For full reasoning on each decision, see `docs/infra/decisions.md`

- [Decision]: [one-line rationale]
- [Decision]: [one-line rationale]
- [Decision]: [one-line rationale]

---

## 5. Conventions

> The Coding Agent must follow these at all times. These are non-negotiable.

### Naming

- [e.g. Files: kebab-case. Classes: PascalCase. Functions: camelCase]
- [e.g. Database collections/tables: snake_case]
- [e.g. Environment variables: SCREAMING_SNAKE_CASE]
- [Add project-specific naming rules]

### Code Style

- Write as little code as possible to accomplish the task.
- Only do things you are more than 90% sure about. If unsure, use the AskUserQuestion tool to ask a series of MCQ questions before writing any code.
- No over-complication. Prefer simple, obvious solutions over clever abstractions. If a simpler approach exists, take it.

### Code Structure

- [e.g. One module per feature. Routes → Service → Repository. No business logic in routes.]
- [e.g. All async functions must handle errors explicitly — no unhandled promise rejections]
- [e.g. No direct database calls outside repository layer]
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

- [e.g. src/auth/middleware.ts — authentication gate for all routes]
- [e.g. src/billing/stripe-webhook.ts — handles payment events]
- [e.g. migrations/ — database schema changes are irreversible in production]
- **Living rule:** When a file or path is identified as load-bearing, add it here immediately with a one-line description of why it's critical.

### File Navigation

- For files exceeding ~500 lines, add a navigation comment block at the top of the file listing key sections with line ranges. Keep it updated when the file changes significantly.
- Format: `// === NAVIGATION === // L1-50: Exports and types // L120-200: Core processing // L450-500: Error handling`
- **The Coding Agent: when reading a file >500 lines, read only the first 50 lines first to check for a `NAVIGATION` block. Use it to read only the relevant section instead of the full file.**

### Error Handling

- [e.g. All errors return { error: string, code: string } — never raw exceptions to client]
- [e.g. Log errors with context before returning to client]
- **Living rule:** When a project-wide error handling pattern is established, add it here immediately.

### Testing

- Full testing philosophy, taxonomy, and workflow: see `docs/infra/testing.md` — read it before writing any tests.
- Three layers required: unit (pure logic), integration (routes → service → database), flow (multi-step user journeys).
- Every endpoint needs: happy path, input validation, and authorization tests at minimum.
- Mock only at the external boundary (email, payment, third-party APIs). Never mock your own database in integration tests.
- Test names read as user-facing descriptions: `"returns 403 when agent accesses another agent's contact"`.
- Before opening a PR, invoke the `pre-merge-qa-tester` agent and reconcile its checklist against the test suite.
- **Living rule:** When a project-specific test convention, runner command, or framework rule is established, add it here immediately.

### Git

- [e.g. Branch naming: feature/*, fix/*, chore/*]
- [e.g. Commit format: conventional commits — feat:, fix:, chore:, docs:]
- [e.g. Never commit directly to main]

### Other

- [Any other conventions critical to this project]

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
# 1. Install dependencies
[install command]

# 2. Copy environment file and fill in values
cp .env.example .env

# 3. [Any database setup, migrations, seeds]
[command]

# 4. Start development server
[dev command]
```

**Key scripts:**

- `[command]` — [what it does]
- `[command]` — [what it does]
- `[command]` — [what it does]

---

## 8. Feature Documentation Index

> Each feature has its own doc in docs/features/. Read the relevant doc before working on a feature.
> When a feature doc exceeds ~400 lines, it is promoted to a directory (docs/features/[feature]/).

> When this index exceeds 50+ features, group rows by domain (e.g. Auth & Identity, Contacts, Billing).

| Feature        | Doc                                                            | Status                    |
| -------------- | -------------------------------------------------------------- | ------------------------- |
| [Feature name] | [docs/features/feature-name.md](docs/features/feature-name.md) | [Stable / WIP / Outdated] |
| [Feature name] | [docs/features/feature-name.md](docs/features/feature-name.md) | [Stable / WIP / Outdated] |

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
