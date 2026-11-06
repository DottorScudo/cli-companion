---
name: release-prep
description: Prepare a release of this project (version bump, changelog and release notes from the merged work) using only its documented release commands, publishing nothing without confirmation. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Release Prep

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task asks to prepare a
release: a new version number, its changelog or release notes, a build of the release.

## Rules

- First find the release instructions of the repository (a release guide, the README, a
  contributing guide, release scripts or the files that hold the version). Without release
  instructions in the repository, change nothing and finish the task as `failed`, asking the
  user to document the release process (or to give this bot a skill with it).
- Version bump and changelog come from the commits merged since the last release: read them
  with git, never invent an entry, and follow the versioning scheme and changelog format the
  project already uses.
- Run only the build and publish commands the project documents. Never guess one, and never
  work around a documented step that fails.
- Publishing, remote tags and deploys only after a confirmation obtained with `ask_user`, with a
  summary of what will happen: the version, the changelog entry, the exact commands and where
  they publish. No answer, or any answer other than a clear yes, means do not publish.
- Prepare locally whatever needs no confirmation (version files, changelog, release notes, a
  local build); a local tag only if the release instructions create one.

## Workflow

1. Read the release instructions and find the last release (tag, changelog entry, version).
2. List the commits merged since then and group them as the changelog does.
3. Choose the new version with the project's scheme; when the scheme leaves it open, ask.
4. Update the version files, the changelog and the release notes; run the documented checks.
5. If the task asks to publish, ask with `ask_user` and only then run the documented commands.

## Report

In the `summary`: the new version and why, the release instructions followed, the files changed,
the commands run with their result, and what was published or left for the user to publish
(with the commands to do it).
