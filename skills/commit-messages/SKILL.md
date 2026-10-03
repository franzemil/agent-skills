---
name: commit-messages
description: >-
  Write Conventional Commits style git commit messages. Use when creating commits,
  squashing, or rewording — enforces type(scope): subject format, imperative mood and
  50/72 line rules.
---

# Conventional Commits

Format every commit message as:

```
<type>(<optional scope>): <imperative subject>

[optional body]
[optional footer(s)]
```

## Types

- `feat` — new feature for the user
- `fix` — bug fix
- `docs` — documentation only
- `refactor` — code change that neither fixes a bug nor adds a feature
- `test` — adding or correcting tests
- `chore` — tooling, dependencies, build
- `perf` — performance improvement

## Rules

- Subject line: imperative mood ("add", not "added"/"adds"), ≤ 50 chars, no trailing period.
- Body wraps at 72 chars; explain *what* and *why*, not *how*.
- Reference issues or breaking changes in the footer (`BREAKING CHANGE:`, `Closes #12`).
- One logical change per commit — split unrelated changes into separate commits.

## When staging commits

Suggest the commit message proactively after completing a coherent change, and show
the proposed message before running `git commit`.
