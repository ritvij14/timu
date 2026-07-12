# PRD: Mobile AI Coding App for SSH/Tmux Workflows

## 1. Product Summary

An open-source mobile app that lets users connect to an SSH-accessible machine and run persistent AI coding sessions through `tmux`.

The app should make coding from a phone feel as simple as chatting with someone on WhatsApp, while still using real developer infrastructure: VPS/local machines, SSH, tmux, Git, dev servers, and coding agent CLIs.

## 2. Core Idea

Users bring their own machine.

The app provides:

* easy SSH connection setup
* persistent connection profiles
* machine readiness checks
* folder/project picker
* tmux-backed coding sessions
* coding agent launcher
* chat-first mobile interface

## 3. Target Users

### Primary

Engineers who already know basic coding and SSH.

### Secondary

AI/vibe coders who may not deeply understand infra, but can follow guided setup and already use tools like Codex, Claude Code, Cursor, etc.

## 4. Product Principles

1. Open source first.
2. Free/self-hostable first.
3. No analytics in V0.
4. No-docs onboarding.
5. Mobile-first, not terminal-first.
6. Terminal available only as advanced mode.
7. Persistent sessions should survive app close, network loss, and phone switching.
8. UX polish can take rough inspiration from Termius, but the app is not a Termius clone.
9. The app should feel like a chat app for coding agents.

## 5. V0 Scope

### Included

* connect to an SSH-accessible machine
* save machine connection profiles
* password auth
* SSH private key auth
* test connection
* machine readiness check
* detect `tmux`
* detect Git/package managers
* detect installed coding agents
* folder/project picker
* start tmux session in selected folder
* launch coding agent inside tmux
* chat-first session UI
* raw terminal fallback

### Excluded

* analytics
* website analytics
* cloud VPS provisioning
* beginner VPS setup flow
* Tailscale/mesh-specific onboarding
* auto-disabling password SSH
* complex WireGuard/Tailscale integration
* full SFTP file editor
* full IDE features
* cloud sync/accounts
* payments
* hosted backend

## 6. Onboarding Flow

### Step 1: Connect a Machine

Main CTA:

**Connect a machine**

Fields:

* machine name
* hostname or IP
* SSH username
* SSH port, default `22`
* auth method

Host examples:

* `123.45.67.89`
* `192.168.1.20`
* `my-mac.local`

Helper text:

> Use your VPS IP, local IP, or `.local` hostname.

There should not be separate flows for VPS and local machines. Technically, both use SSH.

### Step 2: Choose Auth Method

Supported V0 auth methods:

* password login
* paste private key
* import private key file
* optional private key passphrase

Later:

* generate app key pair
* copy public key command
* SSH agent support

### Step 3: Test Connection

The app tests SSH connection.

Success state:

> Connected successfully.

Failure state should be clear and actionable:

* wrong host
* wrong username
* wrong password/key
* port unreachable
* network unavailable
* permission denied

### CLI Pairing Onboarding (`npx timu-app`)

The `npx timu-app` one-time SSH pairing ceremony is an alternative onboarding path to the manual connection form above. The CLI side (`timu-pair` Rust binary + `timu-npx` launcher) is feature-complete and automated-tested (56 tests passing). The following items remain undone and block real-world use:

3. **macOS Remote Login enablement unverified on real hardware** — the `sudo systemsetup -setremotelogin on` code path exists and is tested through the `System` trait seam, but has never been run on a Mac with Remote Login disabled. The sudo authorization dialog path is untested.
4. **VPS acceptance not done** — `--host`/`--user`/`--port` override parsing is tested, but no one has run `npx timu-app --host <vps> --user <user> --port <port>` against a real VPS and completed pairing.
5. **linux-arm64 target has zero test coverage** — CI builds it via cross-compilation and explicitly skips tests (`if: ${{ !matrix.cross }}`).
6. **npm smoke test uses a fake binary, not the real release** — `test/smoke.test.js` validates `npm pack` + install + launcher with a controlled fake binary. It does not test the actual `postinstall.js` download path against a real GitHub release.

The mobile client side (QR scanner, SSH transport, permanent-key reconnect, profile save) is also not implemented but is tracked separately under the app feature work, not here.

## 7. Machine Readiness Check

After SSH connection succeeds, app checks the machine.

Check for:

* `tmux`
* `git`
* shell
* `node`
* `npm`
* `pnpm`
* `yarn`
* `bun`
* coding agent CLIs:

  * `codex`
  * `claude`
  * `opencode`

Display as simple status list:

```txt
tmux        Missing
git         Ready
node        Ready
pnpm        Ready
codex       Ready
claude      Missing
```

## 8. tmux Requirement

`tmux` is required for V0.

If missing, show:

> tmux is needed to keep coding sessions alive after you close the app.

V0 action:

**Show install command**

Later action:

**Install for me**

Avoid risky auto-install in V0.

## 9. Folder Picker

The app needs a good SFTP-like folder picker, but not a full file manager in V0.

Required:

* browse folders
* choose working directory
* show recent folders
* show favorite folders
* show Git repo indicator
* search folders if practical

Example:

```txt
~/projects/kendal-crm        Git repo
~/experiments/mobile-agent   Git repo
~/test-app                   Folder
```

## 10. Agent Picker

After folder selection, user chooses what to run.

Options:

* Codex
* Claude Code
* OpenCode
* Terminal

Only installed agents should appear as ready.

If no agents are installed:

> No coding agent found on this machine.

Then show install guidance.

## 11. Starting a Session

When user starts a session:

1. app creates or reuses a tmux session
2. sets working directory
3. launches selected agent
4. opens chat UI

Conceptually:

```txt
tmux new-session -s <session-id> -c <folder>
```

Then run selected agent inside that tmux session.

Session card example:

```txt
kendal-crm
Codex
Running
Last active 2 min ago
```

## 12. Chat Interface

The main UX should be chat-first.

It should include:

* message input at bottom
* agent replies as chat-style messages
* terminal output collapsed by default
* visible running status
* reconnect button
* raw terminal button
* common action buttons

Useful action buttons:

* Run dev server
* Show errors
* Git status
* Commit
* Deploy
* Stop command

Main rule:

> tmux is the backend, chat is the frontend.

## 13. Connection Persistence UX

Inspired loosely by Termius.

The app should persist:

* saved machines
* auth method
* recent folders
* active sessions
* last opened session
* connection state
* reconnect state

Expected behavior:

* app can close and reopen without losing sessions
* phone network can drop and reconnect
* tmux session keeps running on server
* user can resume from session list

## 14. Security and Privacy

V0 should be local-first.

No analytics.

No account required.

Private keys should be stored securely using platform secure storage.

Do not collect:

* code
* prompts
* terminal output
* repo names
* file paths
* server IPs
* SSH usernames
* command contents

## 15. Analytics Decision

V0 has no analytics.

Early feedback will come from:

* GitHub issues
* GitHub discussions
* direct users
* community feedback

Telemetry can be designed as a stubbed layer, but should do nothing in V0.

Possible future events:

* machine connected
* tmux session started
* agent launched
* deploy action used
* reconnect succeeded/failed

Future analytics should be optional, anonymous, documented, and self-hostable.

## 16. V0 User Flow

```txt
Open app
→ Connect a machine
→ Choose auth
→ Test SSH connection
→ Save machine
→ Check readiness
→ Fix tmux if missing
→ Pick folder
→ Pick agent
→ Start tmux session
→ Chat with coding agent
```

## 17. V0 Success Criteria

V0 is successful if a user can:

1. connect their SSH machine from mobile
2. save the connection
3. detect whether `tmux` and agents are installed
4. choose a project folder
5. start a tmux-backed agent session
6. close the app
7. reopen the app
8. resume the same session
9. send messages through a simple chat UI

## 18. Product Positioning

Not:

> A mobile terminal app.

Not:

> A full cloud coding IDE.

Not:

> A beginner VPS setup tool.

Better:

> An open-source mobile app for running persistent AI coding sessions on your own machine.

Even shorter:

> WhatsApp-style mobile UX for coding agents running on your VPS.

## 19. Open Questions (Rust Core — `timu-core`)

These need to be resolved before or during implementation of the Rust core layer. They shape the architecture, not just the implementation.

1. **tmux output streaming strategy** — `tmux capture-pane` polling vs `tmux pipe-pane` (pipe to a file + tail) vs tmux control-mode (`tmux -C`). Control-mode is the "correct" way but complex; polling is simplest for V0. Needs a quick spike to decide.
2. **SSH connection model** — one long-lived SSH connection per machine with multiple channels (cheaper, survives better) vs one connection per session. russh handles multichannel well; lean is one connection per machine, with tmux interactions as channels.
3. **FFI mechanism to Expo** — UniFFI (generates Kotlin + Swift bindings, mature, used by Signal/FF) vs react-native-rust-style alternatives. And who owns the Expo native module wrapper that calls the generated bindings.
4. **Chat-message extraction from terminal output** — agents don't emit structured messages over a plain PTY. V0 likely treats terminal output as one collapsible stream, with the user's input and the agent's latest reply-block as pseudo-messages. This is a product + Rust collaboration.
5. **Where persistent state lives** — Rust-owned SQLite (rusqlite) for profiles/sessions/folders vs delegating to RN's AsyncStorage. Lean is Rust-owned so session/bookmark data stays consistent and survives RN reinstalls.
