//! Discovery and validation of skills (skills/<name>/SKILL.md) and subagents
//! (subagents/<name>.md) from the repo root.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::model::{Item, Skill, Subagent};

/// Parsed frontmatter of a SKILL.md / subagent markdown file.
#[derive(Debug, Default, serde::Deserialize)]
struct Frontmatter {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Option<Vec<String>>,
}

/// A discovered item or the reason it was rejected.
pub enum ScanOutcome {
    Ok(Item),
    Invalid { path: PathBuf, reason: String },
}

/// Split a markdown file into (frontmatter YAML, prompt body).
fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let rest = raw.strip_prefix("---\n").or_else(|| raw.strip_prefix("---\r\n"))?;
    // find closing delimiter at line start
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            let fm = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some((fm, body));
        }
        offset += line.len();
    }
    None
}

/// Public wrapper for cross-module tests.
pub fn split_frontmatter_for_tests(raw: &str) -> Option<(&str, &str)> {
    split_frontmatter(raw)
}

fn parse_frontmatter(fm: &str) -> Result<Frontmatter> {
    let parsed: Frontmatter =
        serde_yaml::from_str(fm).context("frontmatter is not valid YAML")?;
    Ok(parsed)
}

/// Find the repo root holding `skills/` and `subagents/`:
/// 1. `$AGENT_SKILLS_HOME` when set and valid (used by install.sh),
/// 2. nearest ancestor of `start` containing both dirs (running from the repo / `make`),
/// 3. `~/.agent-skills` clone (binary on PATH, run from any project),
/// 4. error otherwise.
pub fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let has_content = |p: &Path| p.join("skills").is_dir() && p.join("subagents").is_dir();
    if let Some(home) = std::env::var_os("AGENT_SKILLS_HOME") {
        let p = PathBuf::from(home);
        if has_content(&p) {
            return Ok(p);
        }
    }
    let mut dir: &Path = start;
    loop {
        if has_content(dir) {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(".agent-skills");
        if has_content(&p) {
            return Ok(p);
        }
    }
    bail!(
        "could not locate the agent-skills repo (no ancestor of {} contains both \
         `skills/` and `subagents/`, and no ~/.agent-skills clone was found — \
         set AGENT_SKILLS_HOME or run the installer from the repo)",
        start.display()
    )
}

/// Scan the repo and return every item, valid or not (invalid items are reported, not loaded).
pub fn scan_repo(root: &Path) -> Vec<ScanOutcome> {
    let mut out = Vec::new();
    out.extend(scan_skills(&root.join("skills")));
    out.extend(scan_subagents(&root.join("subagents")));
    out
}

/// Only valid items (log invalid ones to stderr).
pub fn scan_valid(root: &Path) -> Vec<Item> {
    scan_repo(root)
        .into_iter()
        .filter_map(|o| match o {
            ScanOutcome::Ok(item) => Some(item),
            ScanOutcome::Invalid { path, reason } => {
                eprintln!("warning: skipping {}: {}", path.display(), reason);
                None
            }
        })
        .collect()
}

fn scan_skills(dir: &Path) -> Vec<ScanOutcome> {
    if !dir.is_dir() {
        return vec![ScanOutcome::Invalid {
            path: dir.to_path_buf(),
            reason: "skills directory not found".into(),
        }];
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(2)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        if entry.path() == dir {
            continue; // the skills/ root itself
        }
        out.push(match read_skill(entry.path()) {
            Ok(skill) => ScanOutcome::Ok(Item::Skill(skill)),
            Err(e) => ScanOutcome::Invalid {
                path: skill_md,
                reason: format!("{e:#}"),
            },
        });
    }
    out
}

fn read_skill(dir: &Path) -> Result<Skill> {
    let skill_md = dir.join("SKILL.md");
    let raw = std::fs::read_to_string(&skill_md)
        .with_context(|| format!("cannot read {}", skill_md.display()))?;
    let (fm, _body) = split_frontmatter(&raw)
        .with_context(|| "missing or unterminated YAML frontmatter (--- blocks)")?;
    let fm = parse_frontmatter(fm)?;
    let name = fm.name.trim().to_string();
    if name.is_empty() {
        bail!("frontmatter field `name` is required and must not be empty");
    }
    if fm.description.trim().is_empty() {
        bail!("frontmatter field `description` is required and must not be empty");
    }
    Ok(Skill {
        name,
        description: fm.description.trim().to_string(),
        path: dir.to_path_buf(),
    })
}

fn scan_subagents(dir: &Path) -> Vec<ScanOutcome> {
    if !dir.is_dir() {
        return vec![ScanOutcome::Invalid {
            path: dir.to_path_buf(),
            reason: "subagents directory not found".into(),
        }];
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(2)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let path = entry.path().to_path_buf();
        out.push(match read_subagent(&path) {
            Ok(agent) => ScanOutcome::Ok(Item::Subagent(agent)),
            Err(e) => ScanOutcome::Invalid {
                path: path.clone(),
                reason: format!("{e:#}"),
            },
        });
    }
    out
}

fn read_subagent(path: &Path) -> Result<Subagent> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let (fm, body) = split_frontmatter(&raw)
        .with_context(|| "missing or unterminated YAML frontmatter (--- blocks)")?;
    let fm = parse_frontmatter(fm)?;
    let name = if fm.name.trim().is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    } else {
        fm.name.trim().to_string()
    };
    if name.is_empty() {
        bail!("cannot determine agent name (frontmatter `name` or filename)");
    }
    if fm.description.trim().is_empty() {
        bail!("frontmatter field `description` is required and must not be empty");
    }
    Ok(Subagent {
        name,
        description: fm.description.trim().to_string(),
        model: fm.model.filter(|m| !m.trim().is_empty()),
        tools: fm.tools.filter(|t| !t.is_empty()),
        body: body.trim_start_matches(['\r', '\n']).to_string(),
        path: path.to_path_buf(),
    })
}

/// Validate the whole repo: every item parses, names are unique across kinds.
/// Returns human-readable problems (empty = valid).
pub fn validate_repo(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for outcome in scan_repo(root) {
        match outcome {
            ScanOutcome::Ok(item) => {
                let key = format!("{}:{}", item.kind(), item.name());
                if let Some(prev) = seen.get(item.name()) {
                    problems.push(format!(
                        "duplicate name '{}' ({} collides with {})",
                        item.name(),
                        key,
                        prev
                    ));
                } else {
                    seen.insert(item.name().to_string(), key);
                }
            }
            ScanOutcome::Invalid { path, reason } => {
                problems.push(format!("{}: {}", path.display(), reason));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_frontmatter() {
        let raw = "---\nname: x\ndescription: y\n---\nBody line\n";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert!(fm.contains("name: x"));
        assert_eq!(body, "Body line\n");
    }

    #[test]
    fn handles_crlf_and_dash_lines() {
        let raw = "---\r\nname: x\r\ndescription: y\r\n---\r\nBody\r\n";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert!(fm.contains("name: x"));
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn rejects_missing_close() {
        assert!(split_frontmatter("---\nname: x\n").is_none());
    }

    #[test]
    fn rejects_missing_open() {
        assert!(split_frontmatter("name: x\n---\n").is_none());
    }

    #[test]
    fn ignores_separator_in_body() {
        let raw = "---\nname: x\n---\nbefore\n---\nafter\n";
        let (fm, body) = split_frontmatter(raw).unwrap();
        assert!(fm.contains("name: x"));
        assert_eq!(body.trim(), "before\n---\nafter");
    }
}
