---
name: tmux-operator
description: Drive a live tmux session from inside Claude Code (which must itself be running inside tmux). Discover existing sessions/panes and continue from them; split panes to the right or below — optionally cd'd into a worktree, optionally launching a second `claude` there; send commands into panes; close panes. Never sends keystrokes into the Claude Code pane itself.
allowed-tools: Bash, Read
---

## Prerequisite

This skill only works when Claude Code is running **inside tmux** (Setup A: user ran `tmux`, then `claude` in the first pane). Check on every invocation:

```bash
echo "TMUX=${TMUX:-<unset>}"
```

If `TMUX` is unset, **stop** and give the user the setup steps at the bottom of this file. Do not attempt any pane operations — there is no live session to attach to.

## State to track in conversation

Keep these in mind across invocations (you, the agent, hold them — the skill has no persistent store):
- `claude_pane` — the pane id Claude Code is running in. Capture once at the first split (see below). **Never `send-keys` to it.**
- `wt_panes` — the pane ids you have created for worktrees this session, in order.

## Action: `list` (run this first, always)

Discover ongoing sessions and continue from them — never assume an empty slate.

```bash
tmux list-sessions
tmux list-panes -t "$TMUX_TARGET" -F '#{pane_index} #{pane_id} #{pane_current_command} #{pane_tty}'
```

`TMUX_TARGET` defaults to the current session: `TMUX_TARGET=$(tmux display-message -p '#{session_name}')`. If the user names a different session, use that. Report the panes in a short table so the user can pick a target.

## Action: `split [right|below] [for <branch>] [claude]`

Open a new pane. `right` = `split-window -h` (side by side), `below` = `split-window -v` (stacked). Default direction is `right`.

1. **Capture `claude_pane` on the first split of the session** (the user is focused on Claude when they hit Enter, so the active pane is Claude's):
   ```bash
   claude_pane=$(tmux display-message -p '#{pane_id}')
   ```
   On later splits, reuse the captured value — do not re-capture (the user may have clicked another pane in the meantime).

2. **Choose the pane to split from.**
   - First worktree pane this session: split from `claude_pane` → new pane appears next to Claude.
   - Subsequent worktree panes: split `below` the most recent entry in `wt_panes` → builds a tidy column of worktree panes to the right of Claude.
   - If the user specifies a target pane (`from %3`), use that instead.

3. **Split and capture the new pane id:**
   ```bash
   new_pane=$(tmux split-window -h -P -F '#{pane_id}' -t "<from_pane>")   # use -v for "below"
   ```
   If the split fails (e.g. pane too small), report the error — do not force.

4. **If `for <branch>` was given, cd the new pane into its worktree.** Compute the path the same way the `worktree` skill does:
   ```bash
   common=$(cd "$(git rev-parse --git-common-dir)" && pwd) && main_root=${common%/*} && parent=${main_root%/*}
   slug=<branch>   # with every '/' replaced by '-'
   wt="$parent/ai-broker-wt-$slug"
   test -d "$wt" && tmux send-keys -t "$new_pane" "cd $wt" Enter || echo "MISSING: $wt"
   ```
   If the worktree doesn't exist, **stop and tell the user to run `/worktree <branch>` first.** Do not create the worktree from this skill.

5. **If `claude` was given, launch a second Claude session in the pane:**
   ```bash
   tmux send-keys -t "$new_pane" "claude" Enter
   ```
   Otherwise leave it as a plain shell.

6. **Record** `$new_pane` in `wt_panes` and confirm: pane id, what it's cd'd into, whether `claude` was launched.

## Action: `send <pane-id> <command...>`

Run a command in a pane by injecting keystrokes (as if the user typed them).

- **Refuse if the target is `claude_pane`** — sending keys there corrupts the Claude TUI. Suggest a worktree pane from `wt_panes` instead.
- Prefer pane ids (`%n`) from `wt_panes` / `list` — they're stable across renumbering.
  ```bash
  tmux send-keys -t "<pane-id>" "<command>" Enter
  ```
- To check what happened, read the pane back:
  ```bash
  tmux capture-pane -t "<pane-id>" -p | tail -20
  ```

## Action: `close <pane-id>`

```bash
tmux kill-pane -t "<pane-id>"
```
Refuse if the target is `claude_pane`. Remove it from `wt_panes` on success. The worktree on disk is untouched — use `/worktree remove <branch>` for that.

## Safety rules (non-negotiable)

- **Never `send-keys` or `kill-pane` into `claude_pane`.** Always check the target against it first.
- **Always `list` before mutating** so you operate on real, current pane state — don't act on stale ids.
- **Never force.** If a split/kill fails, report and stop. No `--force`.
- Worktree creation/removal belongs to the `/worktree` skill. This skill only opens panes inside existing worktrees.

---

## Setup steps to give the user when `TMUX` is unset

Claude Code must run inside tmux for the live-pane workflow. Tell the user:

1. Exit this Claude session (Ctrl-C / `/exit`), or open a new ghostty tab.
2. In the project's main checkout, run:
   ```
   tmux
   ```
3. Inside that tmux pane, start Claude:
   ```
   claude
   ```
4. Now ask for panes again — `$TMUX` will be set and the skill will work. Panes you spawn appear live next to / below this Claude pane, each can run a separate worktree, and you can type into them yourself.