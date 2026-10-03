//! Destination paths per harness and scope.

use std::path::PathBuf;

use crate::model::{Harness, Item, Scope};

/// Where an item should be installed for a given harness + scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// Parent directory that receives the copy (created if missing).
    pub dir: PathBuf,
    /// Final name inside `dir` (skill dir name, or `<agent>.md` for subagents).
    pub leaf: String,
}

impl Destination {
    pub fn full_path(&self) -> PathBuf {
        self.dir.join(&self.leaf)
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME is not set")
}

fn xdg_config() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(".config"),
    }
}

/// Skills destination directory for a harness + scope.
pub fn skills_dir(h: Harness, scope: &Scope) -> PathBuf {
    match (h, scope) {
        (Harness::Pi, Scope::User) => home().join(".pi/agent/skills"),
        (Harness::Pi, Scope::Project(root)) => root.join(".pi/skills"),
        (Harness::Claude, Scope::User) => home().join(".claude/skills"),
        (Harness::Claude, Scope::Project(root)) => root.join(".claude/skills"),
        (Harness::Opencode, Scope::User) => xdg_config().join("opencode/skills"),
        (Harness::Opencode, Scope::Project(root)) => root.join(".opencode/skills"),
    }
}

/// Subagents destination directory for a harness + scope.
pub fn agents_dir(h: Harness, scope: &Scope) -> PathBuf {
    match (h, scope) {
        (Harness::Pi, Scope::User) => home().join(".pi/agent/agents"),
        (Harness::Pi, Scope::Project(root)) => root.join(".agents"),
        (Harness::Claude, Scope::User) => home().join(".claude/agents"),
        (Harness::Claude, Scope::Project(root)) => root.join(".claude/agents"),
        (Harness::Opencode, Scope::User) => xdg_config().join("opencode/agents"),
        (Harness::Opencode, Scope::Project(root)) => root.join(".opencode/agents"),
    }
}

/// Destination for a specific item.
pub fn destination(item: &Item, h: Harness, scope: &Scope) -> Destination {
    match item {
        Item::Skill(s) => Destination {
            dir: skills_dir(h, scope),
            leaf: s.name.clone(),
        },
        Item::Subagent(a) => Destination {
            dir: agents_dir(h, scope),
            leaf: format!("{}.md", a.name),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Skill, Subagent};

    fn skill(name: &str) -> Item {
        Item::Skill(Skill {
            name: name.into(),
            description: "d".into(),
            path: PathBuf::from("/tmp/repo/skills").join(name),
        })
    }

    fn agent(name: &str) -> Item {
        Item::Subagent(Subagent {
            name: name.into(),
            description: "d".into(),
            model: None,
            tools: None,
            body: "prompt".into(),
            path: PathBuf::from("/tmp/repo/subagents").join(format!("{name}.md")),
        })
    }

    #[test]
    fn pi_user_paths() {
        let d = destination(&skill("tdd"), Harness::Pi, &Scope::User);
        assert_eq!(d.full_path(), home().join(".pi/agent/skills/tdd"));
        let d = destination(&agent("reviewer"), Harness::Pi, &Scope::User);
        assert_eq!(d.full_path(), home().join(".pi/agent/agents/reviewer.md"));
    }

    #[test]
    fn claude_project_paths() {
        let root = PathBuf::from("/tmp/proj");
        let d = destination(&skill("tdd"), Harness::Claude, &Scope::Project(root.clone()));
        assert_eq!(d.full_path(), root.join(".claude/skills/tdd"));
        let d = destination(&agent("reviewer"), Harness::Claude, &Scope::Project(root.clone()));
        assert_eq!(d.full_path(), root.join(".claude/agents/reviewer.md"));
    }

    #[test]
    fn opencode_user_paths() {
        let d = destination(&skill("tdd"), Harness::Opencode, &Scope::User);
        assert_eq!(
            d.full_path(),
            xdg_config().join("opencode/skills/tdd")
        );
        let d = destination(&agent("reviewer"), Harness::Opencode, &Scope::User);
        assert_eq!(
            d.full_path(),
            xdg_config().join("opencode/agents/reviewer.md")
        );
    }

    #[test]
    fn pi_project_agents_use_dot_agents() {
        let root = PathBuf::from("/tmp/proj");
        let d = destination(&agent("reviewer"), Harness::Pi, &Scope::Project(root.clone()));
        assert_eq!(d.full_path(), root.join(".agents/reviewer.md"));
    }
}
