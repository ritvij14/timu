---
name: worktree
description: Create or remove a git worktree in the repo's parent folder for isolated parallel work on a branch. Create makes a sibling directory `ai-broker-wt-<slug>` on a new (or existing) branch off `main`, copies `.env`, and runs `yarn install`. Remove deletes the worktree cleanly without forcing and reports whether the branch is safe to delete.
allowed-tools: Bash, Read
---

Argument is a branch name. Decide the action from the user's wording:
- "remove"/"rm"/"delete <branch>" → **Remove** flow.
- otherwise (just a branch name, or "create/add <branch>") → **Create** flow.

The slug for the worktree folder is the branch name with every `/` replaced by `-`.
Example: `feature/voice` → `ai-broker-wt-feature-voice`.

All paths are resolved from the repo, not the current working directory, so this works from any worktree.

---

## Create

1. **Resolve the main checkout root and parent folder** (one command, prints both). The `cd … pwd` absolutizes `--git-common-dir`, which is relative (`.git`) when run from the main checkout:
   ```bash
   common=$(cd "$(git rev-parse --git-common-dir)" && pwd) && main_root=${common%/*} && parent=${main_root%/*} && echo "main=$main_root parent=$parent"
   ```

2. **Compute the worktree path**:
   ```
   slug = <branch> with '/' → '-'
   wt = $parent/ai-broker-wt-<slug>
   ```

3. **Stop if the path already exists**:
   ```bash
   test -e "$wt" && echo "EXISTS: $wt" || echo "ok"
   ```
   If it exists, stop and tell the user — do not overwrite.

4. **Create the worktree**. If the branch already exists, check it out; otherwise create it from `main`:
   ```bash
   if git show-ref --verify --quiet refs/heads/<branch>; then
     git worktree add "$wt" <branch>
   else
     git worktree add -b <branch> "$wt" main
   fi
   ```
   If this fails, stop and show the error — do not retry with `--force`.

5. **Copy `.env`** from the main checkout if it exists (worktrees don't share ignored files):
   ```bash
   test -f "$main_root/.env" && cp "$main_root/.env" "$wt/.env" && echo "env copied" || echo "no .env in main checkout — skip"
   ```

6. **Install dependencies** (each worktree has its own `node_modules`):
   ```bash
   yarn --cwd "$wt" install
   ```

7. **Confirm** in one line: the worktree path, the branch name, and whether `.env` was copied. Remind the user the branch is still tracked after removal (no auto-cleanup) and that they should run `claude` from inside `$wt` for an isolated session.

---

## Remove

1. **Resolve paths** the same way as Create step 1–2.

2. **Confirm the worktree is registered**:
   ```bash
   git worktree list
   ```
   If `$wt` is not listed, stop and tell the user — nothing to remove.

3. **Remove the worktree**:
   ```bash
   git worktree remove "$wt"
   ```
   If this fails because of modified or untracked files, **stop and report** — do not add `--force`. Only re-run with `git worktree remove --force "$wt"` if the user explicitly asks after seeing the warning.

4. **Report on the branch** — is it merged into `main`?
   ```bash
   git merge-base --is-ancestor <branch> main && echo "MERGED" || echo "UNMERGED"
   ```
   - If merged: tell the user it's safe to delete with `git branch -d <branch>`.
   - If unmerged: warn that commits on `<branch>` are not in `main`; offer `git branch -D <branch>` only if they confirm they want to discard them.
   - Do **not** delete the branch automatically.

5. **Confirm** in one line: worktree removed, and the branch status above.