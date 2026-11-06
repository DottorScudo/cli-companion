---
name: code-research
description: Research a technical question about this repository, read-only, and report findings, options and a recommendation. Only for a cli-companion planning task whose bot names this skill; never use it in any other session.
---

# Code Research

## When this applies

Only inside a cli-companion planning task whose bot names this skill. The task asks for an
investigation: how something works, whether a change is feasible, which approach fits best.
Nothing has to change in the project.

## Rules

- Read-only: never create, edit or delete tracked files. Use git only to read (log, show, diff,
  blame, status); never run a git command that changes state.
- Experiments only through the project's own build or test commands, and only when their
  output is ignored by git; check with `git status` that nothing changed afterwards.
- External sources only if you have network access: cite each one (title and link) next to the
  finding it supports. Never present an external claim as verified in this repository.
- Propose tasks only if the request asks for them; otherwise finish with the answer outcome.
- Never guess: mark as unverified what you could not confirm in the code or by an experiment.

## Workflow

1. Restate the question in one sentence and decide what evidence would answer it.
2. Search first (file names, symbols, text, git log), then read the code the search points to.
3. Run an experiment only when reading cannot settle a point.
4. List the realistic options and weigh each against what you found.
5. Stop when the recommendation is supported by the findings.

## Report

When the request asks for no tasks, finish with the answer outcome: the `summary` starts with
"Answer:", in the language of the request, followed by these sections in this order:

- `Question`: the question, in one sentence.
- `Findings`: what you found, with the files (and sources) that support it.
- `Options`: each option with its pros, its cons and its relative effort.
- `Recommendation`: the option you recommend and why.
- `Open points`: what you could not verify and what the user still has to decide.
