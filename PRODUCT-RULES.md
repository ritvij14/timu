# Product Rules — Canonical Spec

**Version 1.0 · July 2026**

This document is the single source of truth for the app's interaction model. It supersedes the model sections of all design-round prompts. The PRD describes *what* the product is; this document describes *how it behaves*. The wireframe export (`Flow Architectures`) is the visual rendering of these rules; where the two conflict, this document wins and the wireframe is the bug.

Every change to this document gets an entry in the Amendment Log (§10). Design or implementation work must reference the version it was built against.

---

## 1. Domain model

**Machine** — an SSH-accessible host the user owns. The app stores connection profiles (name, host, user, port, auth) locally; private keys in platform secure storage.

**Session** — one tmux session bound to exactly one project folder.
- Folder ↔ session is **strictly 1:1**. A folder has at most one app session; a session belongs to exactly one folder.
- A git worktree is its own folder and therefore its own session (`kendal-crm · main` and `kendal-crm · feat/auth` are two sessions).
- The session is the top-level object in the app's main list ("inbox").

**Child** — one tmux window inside a session, rendered full-screen on mobile. Exactly three types:

| Type | What it is | Notes |
|---|---|---|
| `agent` | A coding agent process (Claude Code, Codex, OpenCode) | The privileged child type: drives notifications, default landing target. Multiple agents per session are allowed and supported — no warnings. |
| `command window` | A block-based REPL (§6) | Multiple command windows per session are explicitly allowed; giving a long-running process its own window is the intended way to get it separate at-a-glance status. Nothing may enforce or assume a singleton. |
| `foreign terminal` | Any window/pane the app did not create | Rendered as a generic terminal view: unknown status, still switchable, no rich chrome. |

A session may exist with **zero agents** (e.g. only a dev server) and with **zero running children** (idle, §8).

## 2. tmux mapping

- **Read model: pane = view.** The app is a tmux client (control mode / command polling), not a terminal renderer. It enumerates panes by globally-unique IDs (`%n` panes, `@n` windows, `$n` sessions) and renders each as a full-screen view. tmux's spatial pane layout is irrelevant and is never drawn — no split-screen visuals anywhere.
- **Write model: window = child.** Everything the app creates is a `new-window`, never a `split-window`. Foreign splits (made from a laptop) surface as individual foreign-terminal views.
- **Identification via user options**, not name parsing:
  - Session level: `@app_id` (UUID), `@app_folder`, `@app_created_by`, schema version — set at creation.
  - Window level: `@app_child_type`, `@app_child_id`.
  - A window without app tags is by definition a foreign child.
  - Session name prefix (e.g. `app-kendal-crm`) is a human-readable courtesy only; `@` options are the source of truth.
- User options live in tmux server memory and die with it — acceptable, since the sessions they describe die too. State that must outlive the server (saved machines, recents, and later runbooks) lives in a manifest file on the machine (e.g. `~/.config/<app>/state.json`).
- **V0 scope: app-created sessions only.** Foreign tmux *sessions* are invisible. Discovery + user-selected import of foreign sessions is V0.5/V1 (§9).

## 3. Status & attention

**Precedence ladder** — one global ordering driving inbox sort position, each row's dominant badge, and urgency-ordering of child summaries:

1. `◐` agent needs approval / input
2. `✕` command block failed
3. `✓` agent finished, unread
4. `●` everything running fine
5. `○` idle

A session's inbox position and dominant badge derive from its **worst child**; a child's status derives from its worst (most urgent) block.

**Unread semantics** — when an agent finishes, its child and session row become unread (notified, bold, badged) until the user views that child, after which they are seen/idle. Read state is stored **server-side** (in `@` options / manifest), never device-local, so it survives switching phones.

**Two invasiveness tiers** — precedence controls sorting and badges; invasiveness controls interruption:
- **Interruptive — agent events** (needs approval, needs input, finished): push notification, deep link, may override landing.
- **Passive — command events** (block failed, block finished, process died): re-badge, re-sort, flip the strip dot — but **never move the user, never push**. A failed build may dominate a row's badge while the user stays exactly where they are.

**Nothing closes itself.** A finished or failed block persists — full output, scrollable — until explicitly dismissed. A child persists until explicitly closed. A session persists until explicitly closed (with destructive friction; closing kills the tmux session on the machine). No auto-cleanup, ever.

## 4. Inbox

- **Flat chat list. No section headers, no legend screen, no help section.** WhatsApp-familiar mechanics.
- **Sort:** needs-you sessions pinned to top → everything else by recency of last event → idle sessions muted at the bottom. No other ordering logic.
- **State lives in row anatomy, not list position:**
  - needs-you → bold title + filled badge **with a word on it** (`◐ Approval`, `✕ Failed`)
  - unread finished → bold title + dot badge
  - running → regular weight + quiet status glyph
  - idle → muted row with written state ("Nothing running — recents kept warm")
- Row contents: folder + branch title, machine label, dominant badge, timestamp, and a **comma-separated child summary** ordered by the precedence ladder, truncating with an ellipsis — whatever survives truncation is what matters most. Window order applies only when every child is idle and seen.
- **"While you were away" digest banner** on cold open is the single line of explicit orientation.
- Badges carry **words**, not a glyph vocabulary. If a state would need a legend, put the word on the badge instead.
- **Badge tap vs row tap:** tapping a row follows the resume rules (§5); tapping the row's badge deep-links to the child that owns it. Passive tier still never moves the user *uninvited* — a tap is an invitation.

## 5. In-session navigation

- **Child strip at the top**: every child as name/glyph + status dot; orientation and direct jumps; must survive the keyboard being open.
- **Horizontal swipe is the primary switching mechanism**, works anywhere on screen. The carousel uses **stable spatial order** (muscle memory), deliberately different from the inbox summary's urgency order — the two orderings serve different jobs and must not be unified.
- **Resume:** opening a session lands on the last-active child (recorded when the user leaves), **unless** a child needs-you — then land there, with one-tap return to the last-active child. If multiple children need attention: land on the highest-precedence one; the header shows a passive count for the rest — an available hop, never a forced tour.
- **Notifications deep-link** to the child that fired them, not the last-active child.

## 6. Command window (block-based REPL)

- Composer at the bottom; the user types shell commands themselves. No guided runner, no picker, no step UI in V0.
- Each command + its full output is a discrete scrollable **block** with a state: `running` (live tail) / `exited ✓` / `exited ✕ (code)`.
- **Full output retained per block. No error summarization, extraction, or interpretation.** The block boundary is the error context; the user reads what the terminal printed.
- A long-running process (dev server, test watcher) is a block that hasn't exited, pinned live-tailing at the bottom until stopped.
- **Port chip:** when a running block has a detected port, the block shows a small chip (`:5173 · copy URL / open in browser` → system browser). This is the entire preview story in V0 — **no in-app browser, no webview, no tunnel UI anywhere.**
- Stopping a running block: hold + confirm (destructive friction). Dismissing a finished block: cheap.
- The most urgent block rolls up to the child's strip dot and the inbox summary entry (`cmd ✕ exit 1`) via the ladder — the blocks model changes in-window rendering only; the attention system is untouched.

## 7. Onboarding & readiness (V0, per PRD)

- Connect machine: name, host/IP, user, port (default 22); auth = password, pasted key, or imported key file (+ optional passphrase). One flow for VPS and local — no separate paths.
- Test connection with specific, actionable failure states (wrong host / user / credentials, port unreachable, network down, permission denied).
- Readiness check: tmux, git, shell, node/npm/pnpm/yarn/bun, agent CLIs (`claude`, `codex`, `opencode`). tmux missing blocks with a one-tap copyable install command — never auto-install in V0.
- Folder picker: browse, recents, favorites, git-repo indicator. Recognition everywhere — hostnames, folders, and agents are always shown and selected, never recalled and typed.

## 8. Lifecycle

- Idle session (all children closed): stays in the inbox, muted, with written idle state; recents and (later) runbooks stay warm. Re-entering offers starting an agent or command window.
- "New session" on a folder that already has one silently becomes "open existing" (1:1 rule) — the picker copy carries that weight.
- Closing a session: explicit, destructive friction, states clearly that the tmux session on the machine will be killed.
- Disconnect: child views show a disconnected state; reconnect catches status up; the design must make it emotionally obvious that everything kept running server-side and nothing was lost.

## 9. Out of scope for V0 (parking lot)

- **Saved command sequences / runbooks** — V1. Direction upgraded by the blocks model: extract a runbook from an actual run of blocks ("save this run as…"), rather than authoring steps in a form. The parked sequence-runner wireframes are superseded, not just deferred.
- **In-app browser preview / tunneling** — cut from V0 entirely; port chip only.
- **Foreign session discovery + import** — V0.5/V1: list non-app sessions read-only, let the user select which to adopt; the app then tags them (`@app_id` etc.) to bring them under management.
- Per PRD: no analytics, no accounts, no cloud backend, no VPS provisioning, no auto-install, no full SFTP editor.

## 10. Amendment log

| Ver | Date | Change |
|---|---|---|
| 1.0 | 2026-07 | Initial canonical spec. Consolidates: convergence-round fixed model; blocks-model amendment (3 child types, command window as REPL, preview → port chip); headerless WhatsApp-style inbox with worded badges; badge-tap vs row-tap rule; explicit multiple-command-windows allowance; tmux `@`-option identification scheme; V0 app-created-sessions-only posture. |

**Known tensions to watch in testing** (from the design punch-list): users may expect a `✕ Failed` row tap to land on the failure (mitigated by badge-tap rule); a single command window shows only its worst block's status on the strip (mitigated by multiple windows); dual orderings (urgency in summaries, spatial in carousel) carry a small reorientation cost.
