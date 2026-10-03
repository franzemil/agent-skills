#!/usr/bin/env bash
# Plain-bash fallback installer used by install.sh when neither a prebuilt
# binary nor cargo is available. Asks harness + scope, copies everything
# (no item-level selection, no TUI), adapts opencode agents minimally.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }

echo
echo "Install agent-skills (bash fallback — installs EVERYTHING)"
echo
echo "Harness:"
echo "  1) pi        2) claude     3) opencode     4) all"
printf "Choice [1-4]: "
read -r harness
echo
echo "Scope:"
echo "  1) Global (user-level)     2) Local (current directory: $(pwd))"
printf "Choice [1-2]: "
read -r scope

case "$scope" in
    2) SCOPE_KIND="project" ;;
    *) SCOPE_KIND="user" ;;
esac

dirs_for() { # harness -> skills_dir agents_dir
    case "$1:$SCOPE_KIND" in
        pi:user)     echo "$HOME/.pi/agent/skills $HOME/.pi/agent/agents" ;;
        pi:project)  echo ".pi/skills .agents" ;;
        claude:user)    echo "$HOME/.claude/skills $HOME/.claude/agents" ;;
        claude:project) echo ".claude/skills .claude/agents" ;;
        opencode:user)    echo "$HOME/.config/opencode/skills $HOME/.config/opencode/agents" ;;
        opencode:project) echo ".opencode/skills .opencode/agents" ;;
    esac
}

install_into() {
    local h="$1"
    read -r skills_dir agents_dir <<< "$(dirs_for "$h")"
    say "[$h] skills -> $skills_dir, agents -> $agents_dir"
    mkdir -p "$skills_dir" "$agents_dir"
    for skill_dir in "$REPO_DIR"/skills/*/; do
        [ -f "$skill_dir/SKILL.md" ] || continue
        name="$(basename "$skill_dir")"
        if [ -e "$skills_dir/$name" ] && [ ! -f "$skills_dir/.agent-skills.json" ]; then
            echo "  skip $name (exists and not managed by agent-skills)"
            continue
        fi
        rm -rf "$skills_dir/$name"
        cp -R "$skill_dir" "$skills_dir/$name"
        echo "  installed skill $name"
    done
    for agent_md in "$REPO_DIR"/subagents/*.md; do
        [ -f "$agent_md" ] || continue
        name="$(basename "$agent_md" .md)"
        target="$agents_dir/$name.md"
        if [ -e "$target" ]; then
            echo "  skip $name (exists)"
            continue
        fi
        if [ "$h" = "opencode" ]; then
            # insert `mode: subagent` after the description line, drop `name:`/`tools:`
            sed -e '/^name:/d' -e '/^tools:/d' -e '/^description:/a\
mode: subagent' "$agent_md" > "$target"
        else
            cp "$agent_md" "$target"
        fi
        echo "  installed agent $name"
    done
}

case "$harness" in
    1) install_into pi ;;
    2) install_into claude ;;
    3) install_into opencode ;;
    4) for h in pi claude opencode; do install_into "$h"; done ;;
    *) echo "invalid choice"; exit 1 ;;
esac

say "Done. Re-run this script to update (managed items are refreshed)."
