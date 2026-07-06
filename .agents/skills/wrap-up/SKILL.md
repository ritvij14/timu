---
description: End-of-session wrap-up. Run this before closing any agent session.
allowed-tools: Bash, Edit, Write, Read
---

The Stop hook has already printed the changed file list (`=== changed files ===`). Use that output — do not run `git status` again. Use your session context to decide what needs updating. Trivial edits, finishing touches, and minor fixes need nothing — just clear the flag and move on.

**Feature docs** — update only if behavior, architecture, or data model changed. if its a feature for which a new documentation page needs to be created, do so. But only if the feature is not already documented and big enough to warrant a dedicated page.

**ADR** (`docs/infra/decisions.md`) — only for real architectural decisions (technology choice, new cross-cutting pattern, structural change). Not for implementation details.

**Infra Docs** — update only if infrastructure related changes have been made.

**TODO** — add deferred work; remove completed items. Skip if neither applies.

Always run `rm -f .agents/session-changed` at the end, even if you skipped everything.
For Codex, `.agents/` is read-only in the default sandbox even when the project
root is writable. Codex must request escalated permission for this removal on
the first attempt; do not try the sandboxed command first.

Confirm in one line: what was updated, or why skipped.
