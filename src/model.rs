//! Core types: skills, subagents, harnesses, scopes.

use std::fmt;
use std::path::PathBuf;

/// Supported harnesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Pi,
    Claude,
    Opencode,
}

pub const ALL_HARNESSES: [Harness; 3] = [Harness::Pi, Harness::Claude, Harness::Opencode];

impl Harness {
    pub fn key(&self) -> &'static str {
        match self {
            Harness::Pi => "pi",
            Harness::Claude => "claude",
            Harness::Opencode => "opencode",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Harness::Pi => "pi",
            Harness::Claude => "Claude Code",
            Harness::Opencode => "opencode",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "pi" | "pidev" => Ok(Harness::Pi),
            "claude" | "claude-code" => Ok(Harness::Claude),
            "opencode" => Ok(Harness::Opencode),
            other => Err(anyhow::anyhow!(
                "unknown harness '{other}' (expected pi, claude or opencode)"
            )),
        }
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Install scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// User-level (e.g. ~/.claude/skills)
    User,
    /// Project-level (e.g. <project>/.claude/skills)
    Project(PathBuf),
}

impl Scope {
    pub fn key(&self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Project(_) => "project",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Scope::User => "Global (user-level)".to_string(),
            Scope::Project(p) => format!("Local ({})", p.display()),
        }
    }
}

/// A generic skill: directory containing SKILL.md.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill name from frontmatter (must match dir name loosely; frontmatter wins).
    pub name: String,
    pub description: String,
    /// Absolute path of the skill directory (source of the copy).
    pub path: PathBuf,
}

/// A generic subagent: markdown file with frontmatter + prompt body.
#[derive(Debug, Clone)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    /// Optional model hint, passed through to harnesses that support it.
    pub model: Option<String>,
    /// Optional generic tool list, passed through where supported.
    pub tools: Option<Vec<String>>,
    /// Everything after the closing `---` of the frontmatter (the system prompt).
    pub body: String,
    /// Absolute path of the source markdown file.
    pub path: PathBuf,
}

/// Anything the installer can install.
#[derive(Debug, Clone)]
pub enum Item {
    Skill(Skill),
    Subagent(Subagent),
}

impl Item {
    pub fn name(&self) -> &str {
        match self {
            Item::Skill(s) => &s.name,
            Item::Subagent(a) => &a.name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Item::Skill(s) => &s.description,
            Item::Subagent(a) => &a.description,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Item::Skill(_) => "skill",
            Item::Subagent(_) => "subagent",
        }
    }

    /// Source path (dir for skills, file for subagents).
    pub fn source(&self) -> &std::path::Path {
        match self {
            Item::Skill(s) => &s.path,
            Item::Subagent(a) => &a.path,
        }
    }
}
