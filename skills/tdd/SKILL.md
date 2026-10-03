---
name: tdd
description: >-
  Strict test-driven development workflow. Use when writing or modifying code that has
  (or should have) unit tests — drives red/green/refactor cycles, one failing test at a
  time, and refuses implementation before a failing test exists.
---

# Test-Driven Development

Work in small red/green/refactor cycles:

1. **Red** — write the smallest failing test that captures the next bit of behavior.
   Run the test suite and confirm the new test fails for the expected reason.
2. **Green** — write the minimum implementation that makes the test pass. Do not
   generalize or add unrequested features.
3. **Refactor** — with the suite green, clean up duplication and improve naming.
   Re-run tests after each refactor step.

## Rules

- Never write implementation code before a failing test exists.
- One logical assertion per test; one behavior change per cycle.
- If a test fails unexpectedly, stop adding features and fix it first.
- When fixing a bug, first write a test that reproduces it.
- Keep the full suite green before committing.

## Output style

When applying this skill, narrate each cycle briefly: `RED: <test>`, `GREEN: <change>`,
`REFACTOR: <cleanup>` — then the final test-run summary.
