---
name: code-answer
description: Answer a short question about this repository or its git history, read-only, for a non-technical user. Only for a cli-companion planning task whose bot names this skill; never use it in any other session.
---

# Code Answer

## When this applies

Only inside a cli-companion planning task whose bot names this skill. The task asks a question
about this repository or its git history, and nothing has to change in the project.

## Rules

- Read-only: never create, edit or delete files. Use git only to read (log, show, diff, blame,
  status); never run a git command that changes state.
- Do not propose tasks. If the question really asks for a change, answer what you found and
  suggest, in one line, that the user asks for the change.
- Read the fewest files that answer the question; stop as soon as you can answer.
- Never guess: say what you could not verify.

## Workflow

1. Restate the question to yourself in one sentence and decide what would answer it.
2. Search first (file names, symbols, text, git log), then open only the files the search points to.
3. Stop when the answer is supported by what you read.

## Report

Finish with the answer outcome: the `summary` starts with "Answer:", followed by at most 10 lines
in plain words, in the language of the question. Name files only when the user needs them, and
say what you could not verify.
