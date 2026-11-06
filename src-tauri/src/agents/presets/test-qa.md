---
name: test-qa
description: Write or extend automated tests with the project's existing framework and conventions, keeping them deterministic. Only for a cli-companion work task whose bot names this skill; never use it in any other session.
---

# Test QA

## When this applies

Only inside a cli-companion work task whose bot names this skill. The task asks for tests:
new coverage, missing cases, a flaky test to stabilise, or a check of existing behaviour.

## Rules

- Use the test framework, layout, helpers and naming the project already uses for that code.
  Never add a new test framework or runner unless the specification asks for it.
- Deterministic tests only: no fixed sleeps, no real clock, network or random values without a
  fixed seed or a fake, no dependence on the order of the tests or on leftover data.
- Test observable behaviour through the public interface of the code, not its internals.
- A test that reveals a bug is not "fixed" by changing the production code or by weakening the
  test: keep it only marked as skipped or expected to fail, with the reason, if the framework
  supports it (otherwise leave it out of the suite), and report the bug in `follow_ups` with the
  test that shows it.
- Change production code only when the specification asks for it.

## Workflow

1. Read the code under test and its existing tests; note the conventions and the gaps.
2. List the cases to cover: the main path, the edge cases and the failure paths.
3. Write the tests next to the existing ones, reusing the project's helpers and fixtures.
4. Run the new tests several times and the relevant suites: everything passes every time.

## Report

In the `summary`: the tests added (names and files), the cases they cover and the command that
runs them. In `follow_ups`: every bug a test revealed, with the test that shows it.
