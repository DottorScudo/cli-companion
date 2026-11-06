---
name: bug-fix
description: Fix a reported bug by reproducing it first, correcting its root cause and keeping the reproduction as a regression test. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Bug Fix

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task reports a bug:
something the project does that it should not, or does not do that it should.

## Rules

- Reproduce before touching the code: a test that fails because of the bug or, only when it
  cannot be automated, documented steps with the observed and the expected result.
- Fix the root cause, not the symptom. Do not silence an error, widen a check or special-case
  the reported input just to make the reproduction pass.
- The reproduction stays as a regression test of the project's suite, next to the tests of the
  code it covers.
- If the bug does not reproduce, never fix it blindly: call `ask_user` with what you tried and
  one clear question, or finish the task as `failed` listing the steps you tried.

## Workflow

1. Read the report and find the code involved; note the expected behaviour.
2. Reproduce: write the failing test (or document the manual steps) and run it to see it fail.
3. Find the cause: trace why the code produces the wrong result, until you can explain it.
4. Fix the cause with the smallest change that explains the whole bug.
5. Run the regression test and the relevant suites: the test now passes and nothing else breaks.

## Report

In the `summary`: the cause (why it happened), the fix (what changed and where) and the test
(its name, and that it failed before the fix and passes after). For a bug reproduced only by
hand, the documented steps instead of the test name.
