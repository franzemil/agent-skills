# agent-skills

One repo of **generic skills** and **subagents**, installable into any supported coding
agent harness with a TUI multi-select picker.

Supported harnesses: **pi**, **Claude Code**, **opencode**.

## Quick start

```bash
# one-line bootstrap: clones to ~/.agent-skills, installs the binary, runs the wizard
curl -fsSL https://raw.githubusercontent.com/franzemil/agent-skills/main/install.sh | bash
```

```bash
# or from a clone of this repo
make install            # smart wizard: harness → items → Local/Global scope
make install-pi         # or pick the harness directly
make install-claude
make install-opencode
make install-all        # same selection into every harness
```

Other targets: `make list` (print all items), `make validate` (frontmatter sanity),
`make sync` (re-copy your last selection), `make test`.

## What the wizard does

```
┌ Select harnesses ────────────────────────┐
│ Harnesses                                │
│ [x] pi                                   │
│ [ ] Claude Code                          │
│ [ ] opencode                             │
│ ↑/↓ move · space toggle · a all · …      │
└──────────────────────────────────────────┘
        ↓
┌ Select skills & subagents ───────────────┐
│ Skills (2)                               │
│ [x] tdd        — Strict test-driven…     │
│ [ ] commit-messages                       │
│ Subagents (2)                            │
│ [x] reviewer   — Reviews code for…       │
└──────────────────────────────────────────┘
        ↓
┌ Install scope ───────────────────────────┐
│ > Global (user-level)                    │
│   Local (/path/to/detected/project)      │  ← shown when run inside a project
└──────────────────────────────────────────┘
        ↓  confirm → copy + adapt → result screen
```

Keys: `↑/↓` (or `k/j`) move · `space` toggle · `a` all · `n` none · `Enter` confirm ·
`Esc` back/cancel · `Ctrl-C` abort.

## Repo layout

```
skills/<name>/SKILL.md     generic skills (Agent Skills spec — agentskills.io)
subagents/<name>.md        generic subagents (format below)
```

### Adding a new skill

1. Create `skills/<my-skill>/SKILL.md`:

   ```markdown
   ---
   name: my-skill
   description: What this skill does and when to use it
   ---
   Instructions for the agent…
   ```

   `name` and `description` are required. Optional: `license`, `compatibility`,
   `metadata`. Supporting files (scripts, references) go next to `SKILL.md`.

2. Check it: `make validate` → `make list`
3. Install it: `make install` (or any `make install-<harness>`)
4. Commit — items live in git, installs are snapshots.

### Adding a new subagent

Create `subagents/<my-agent>.md`. Frontmatter + body (body becomes the system prompt):

```markdown
---
name: reviewer
description: Reviews code for quality and best practices
model: claude-sonnet-4        # optional, passthrough
tools: [read, grep, bash]     # optional, generic list
---
You are a code reviewer. Focus on code quality, bugs, and security…
```

At install time the frontmatter is adapted to each harness:

| Harness  | Skills dir (Global → Local)                        | Agents dir (Global → Local)                        | Adaptation |
|----------|----------------------------------------------------|----------------------------------------------------|------------|
| pi       | `~/.pi/agent/skills/` → `.pi/skills/`              | `~/.pi/agent/agents/` → `.agents/`                 | passthrough |
| claude   | `~/.claude/skills/` → `.claude/skills/`            | `~/.claude/agents/` → `.claude/agents/`            | passthrough (keeps `tools`, `model`) |
| opencode | `~/.config/opencode/skills/` → `.opencode/skills/` | `~/.config/opencode/agents/` → `.opencode/agents/` | adds `mode: subagent`, drops `name`/`tools`, filename = agent name |

## Install model

- **Copy-based** — each harness gets a snapshot; your repo stays the source of truth.
- After editing items, re-apply with `make sync` (replays your last selection per
  harness/scope, recorded in `.installer-state.json`).
- **Safe by default** — the installer never overwrites destination files it did not
  create; foreign files are skipped with a warning (`--force` overrides).
- Each destination directory gets a `.agent-skills.json` manifest listing what we
  manage there; managed items are refreshed on re-install.
- Run from inside any project to get the **Local** scope option (detected via git root
  or project markers like `package.json`, `Cargo.toml`, `go.mod`).

## CLI

```
agent-skills [OPTIONS]

  --harness <pi|claude|opencode|all>   Preselect harness (skips picker)
  --scope <auto|user|project>          auto = detect folder and ask (default)
  --items <a,b>                        Explicit selection (paths or names)
  --all-items                          Select everything (no item screen)
  --sync                               Re-apply last selection per harness
  --list                               List items and exit
  --validate                           Validate frontmatter and exit
  --dry-run                            Show what would be done, write nothing
  --no-tui                             Non-interactive (for CI / no TTY)
  --force                              Overwrite unmanaged destination files
```

Non-interactive example:

```bash
agent-skills --harness claude --all-items --scope user --no-tui
```

## Bootstrap details (`install.sh`)

The one-liner:

1. Clones/updates the repo at `~/.agent-skills` (override: `AGENT_SKILLS_HOME`),
2. Gets the binary: prebuilt release asset for your platform (macOS arm64/x86_64,
   Linux arm64/x86_64 musl) → falls back to `cargo build --release` → falls back to a
   plain-bash copy mode (no TUI),
3. Symlinks `agent-skills` into `~/.local/bin` (override: `AGENT_SKILLS_BIN`),
4. Launches the wizard.

Environment overrides: `AGENT_SKILLS_REPO` (clone URL), `AGENT_SKILLS_HOME` (clone
location), `AGENT_SKILLS_BIN` (PATH dir).

## Development

```bash
make build    # cargo build --release
make test     # unit tests (adapters, scanner, installer)
```

Releasing: push a tag — CI builds all four platforms and attaches tarballs + checksums
to the GitHub release; `install.sh` automatically picks the newest tag.

```bash
git tag -a v0.2.0 -m "v0.2.0" && git push origin v0.2.0
```
