---
name: git-ops
description: Perform git maintenance (branch cleanup, conflict resolution, reverting a change) safely, with a backup ref before any destructive step and never rewriting shared history. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Git Ops

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task asks for git
maintenance: delete old branches, resolve a conflict, revert a change, tidy tags or remotes.
Questions about the history that change nothing are not git maintenance: answer them read-only.

## Rules

These rules are blocking: when the task cannot be done within them, ask or stop, never bend them.

- Never force-push (`push --force`, `--force-with-lease`) to a shared branch, and never rewrite
  the history of the main branch or of the base branch of the task (no amend, rebase, reset or
  filter of commits already there). Undo published work with `git revert`.
- Never run `reset --hard`, `clean -fd`, `branch -D`, `stash drop` or delete a remote branch
  without a confirmation obtained with `ask_user`. The question lists exactly what is lost:
  each branch with its unmerged commits, each file or change, each stash entry. No answer, or
  any answer other than a clear yes, means no.
- Before every destructive operation, create a backup ref that keeps what it removes: a tag or a
  local branch named `backup/<date>` (for example `backup/2026-01-31-old-feature`). Never delete
  a backup ref in the same task.
- Run git in the worktree of the task, not in the user's checkout, unless the specification asks
  for the checkout or the shared repository. Touch a remote only when the specification asks
  for it.
- Never use `git stash` without a unique message, and never pop or drop an entry you did not
  create: the stash stack is shared by every worktree and every other session. Prefer a
  temporary commit to set work aside.
- If your work produces no commit of its own (for example only deleting branches), skip the
  commit, push, pull request and merge steps of the delivery and finish the task as
  `completed` without a `commit`, describing in the summary what you did.

## Workflow

1. Read the request and inspect the state: `git status`, `git branch -a -vv`, `git log --graph`,
   `git stash list`, the remotes. Record the state before.
2. Plan the commands and mark the destructive ones; for each, list what it would lose.
3. Ask with `ask_user` for every destructive step, in one question per batch, and wait.
4. Create the backup refs, then run the commands one at a time, checking each result.
5. Inspect the state again and record the state after.

## Report

In the `summary`: the commands run, in order; the state before and after (branches, commits,
stashes, remotes touched); the backup refs created; and how to go back (the exact commands that
restore each change, for example `git branch <name> backup/<date>`).
