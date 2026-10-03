# agent-skills

One repo of **generic skills** and **subagents**, installable into any supported coding
agent harness with a TUI multi-select picker.

Supported harnesses (v1): **pi**, **Claude Code**, **opencode**.

## Quick start

```bash
# from the repo
make install            # smart wizard: folder detection → harness → items → Local/Global
make install-pi         # or pick the harness directly
make install-claude
make install-opencode
make install-all        # same selection into every harness

# one-line bootstrap (clones to ~/.agent-skills, installs binary, runs wizard)
curl -fsSL https://raw.githubusercontent.com/<franzemil/agent-skills/main/install.sh | bash
```

Other targets: `make list` (print all items), `make validate` (frontmatter sanity),
`make sync` (re-copy your last selection), `make test`.

## Repo layout

```
skills/<name>/SKILL.md     generic skills (Agent Skills spec — agentskills.io)
subagents/<name>.md        generic subagents (format below)
```

### Skill format

Standard [Agent Skills](https://agentskills.io/specification) format — a directory per
skill containing a `SKILL.md` with YAML frontmatter:

```markdown
---
name: my-skill
description: What this skill does and when to use it
---
Instructions for the agent…
```

`name` and `description` are required. Optional: `license`, `compatibility`, `metadata`.

### Generic subagent format

One markdown file per subagent. Frontmatter + body (body becomes the system prompt):

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

| Harness  | Skills dir (Global → Local)                         | Agents dir (Global → Local)                          | Adaptation |
|----------|-----------------------------------------------------|------------------------------------------------------|------------|
| pi       | `~/.pi/agent/skills/` → `.pi/skills/`               | `~/.pi/agent/agents/` → `.agents/`                   | passthrough |
| claude   | `~/.claude/skills/` → `.claude/skills/`             | `~/.claude/agents/` → `.claude/agents/`              | passthrough (keeps `tools`, `model`) |
| opencode | `~/.config/opencode/skills/` → `.opencode/skills/`  | `~/.config/opencode/agents/` → `.opencode/agents/`   | adds `mode: subagent`, drops `name`/`tools`, filename = agent name |

Installation is **copy**-based (snapshots). Re-run `make sync` after editing items to
re-apply your last selection. The installer never overwrites files it did not create —
collisions are skipped with a warning (use `--force` to override).

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

## Development

```bash
make build    # cargo build --release
make test     # unit tests (adapters, scanner)
```
