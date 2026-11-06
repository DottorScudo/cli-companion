---
name: safe-refactor
description: Refactor code in small verified steps without changing its behaviour, public API or data formats, with green tests before and after. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Safe Refactor

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task asks to restructure
code (rename, extract, move, split, simplify) while it keeps doing exactly what it does now.

## Rules

- Green tests before you start: run the tests that cover the code you will touch. If they are
  missing, write them first, against the current behaviour, and see them pass.
- Small steps: one refactoring at a time, each followed by the build and the tests. If a step
  breaks them, undo that step instead of patching forward.
- No change of behaviour, public API or data format (files, messages, stored data) unless the
  specification asks for it.
- Do not fix bugs on the way: report them in `follow_ups`, so the refactoring stays a pure one.

## Workflow

1. Read the code to refactor, its callers and its tests; define the target structure.
2. Run the covering tests and see them pass; write the missing ones first.
3. Apply the refactoring as a sequence of small steps, verifying each one.
4. Run the relevant suites at the end and compare the public interface before and after.

## Report

In the `summary`: the list of steps, and the proof of equivalence (the tests that cover the
changed code, those written first, and that they passed before and after every step). Say what
is not covered by any test.
