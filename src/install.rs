//! Copy-based installation with collision safety and state recording.
//!
//! Safety model: every destination directory we touch gets a manifest file
//! (`.agent-skills.json`) listing the leaves we manage. A pre-existing file or
//! directory at a destination leaf that is NOT in the manifest is treated as
//! foreign and skipped with a warning (unless `--force`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::adapt;
use crate::harness;
use crate::model::{Harness, Item, Scope};

pub const MANIFEST_NAME: &str = ".agent-skills.json";
pub const STATE_NAME: &str = ".installer-state.json";

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    pub kind: String,   // "skill" | "subagent"
    pub name: String,
    pub source: String, // repo-relative source path
}

pub type Manifest = BTreeMap<String, ManifestEntry>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateSelection {
    pub harness: String,
    pub scope: String, // "user" | "project:<abs path>"
    pub items: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    #[serde(default)]
    pub selections: Vec<StateSelection>,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub force: bool,
    pub dry_run: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self { force: false, dry_run: false }
    }
}

/// Outcome for one (item, harness) pair.
#[derive(Debug)]
pub enum Outcome {
    /// Installed (or would be, in dry-run).
    Installed { dest: PathBuf },
    /// Destination occupied by a foreign file — skipped.
    SkippedForeign { dest: PathBuf },
    /// Something failed.
    Failed { dest: PathBuf, error: String },
}

#[derive(Debug, Default)]
pub struct InstallReport {
    pub outcomes: Vec<Outcome>,
}

impl InstallReport {
    pub fn installed(&self) -> Vec<&PathBuf> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                Outcome::Installed { dest } => Some(dest),
                _ => None,
            })
            .collect()
    }

    pub fn summary(&self) -> String {
        let n_installed = self.outcomes.iter().filter(|o| matches!(o, Outcome::Installed { .. })).count();
        let n_skipped = self.outcomes.iter().filter(|o| matches!(o, Outcome::SkippedForeign { .. })).count();
        let n_failed = self.outcomes.iter().filter(|o| matches!(o, Outcome::Failed { .. })).count();
        format!(
            "installed: {n_installed}, skipped (foreign): {n_skipped}, failed: {n_failed}"
        )
    }
}

/// Install a set of items into one harness under one scope.
pub fn install_items(
    items: &[Item],
    h: Harness,
    scope: &Scope,
    repo_root: &Path,
    opts: &InstallOptions,
) -> InstallReport {
    let mut report = InstallReport::default();
    // Group by destination dir so each manifest is loaded/saved once.
    let mut by_dir: BTreeMap<PathBuf, Vec<&Item>> = BTreeMap::new();
    for item in items {
        let dest = harness::destination(item, h, scope);
        by_dir.entry(dest.dir).or_default().push(item);
    }
    for (dir, group) in by_dir {
        let mut manifest = read_manifest(&dir);
        for item in group {
            let dest = harness::destination(item, h, scope);
            let leaf = dest.leaf.clone();
            let exists = dest.full_path().exists();
            let managed = manifest.contains_key(&leaf);
            if exists && !managed && !opts.force {
                report.outcomes.push(Outcome::SkippedForeign {
                    dest: dest.full_path(),
                });
                continue;
            }
            if opts.dry_run {
                report.outcomes.push(Outcome::Installed { dest: dest.full_path() });
                continue;
            }
            let res = match item {
                Item::Skill(skill) => copy_dir_recursive(&skill.path, &dest.full_path())
                    .with_context(|| format!("copying skill '{}' into {}", skill.name, dest.full_path().display())),
                Item::Subagent(agent) => std::fs::create_dir_all(&dir)
                    .and_then(|_| {
                        std::fs::write(
                            dest.full_path(),
                            adapt::adapt_subagent(agent, h),
                        )
                    })
                    .with_context(|| format!("writing agent '{}' to {}", agent.name, dest.full_path().display())),
            };
            match res {
                Ok(()) => {
                    manifest.insert(
                        leaf.clone(),
                        ManifestEntry {
                            kind: item.kind().to_string(),
                            name: item.name().to_string(),
                            source: relative_source(item.source(), repo_root),
                        },
                    );
                    report.outcomes.push(Outcome::Installed { dest: dest.full_path() });
                }
                Err(e) => report.outcomes.push(Outcome::Failed {
                    dest: dest.full_path(),
                    error: format!("{e:#}"),
                }),
            }
        }
        if !opts.dry_run && !manifest.is_empty() {
            if let Err(e) = write_manifest(&dir, &manifest) {
                report.outcomes.push(Outcome::Failed {
                    dest: dir.join(MANIFEST_NAME),
                    error: format!("writing manifest: {e:#}"),
                });
            }
        }
    }
    report
}

fn relative_source(src: &Path, repo_root: &Path) -> String {
    src.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| src.display().to_string())
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join(MANIFEST_NAME)
}

fn read_manifest(dir: &Path) -> Manifest {
    std::fs::read_to_string(manifest_path(dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating {}", dir.display()))?;
    let raw = serde_json::to_string_pretty(manifest)
        .context("serializing manifest")?;
    std::fs::write(manifest_path(dir), raw)
        .with_context(|| format!("writing {}", manifest_path(dir).display()))
}

/// Recursive directory copy that overwrites the destination.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    use walkdir::WalkDir;
    if !src.is_dir() {
        anyhow::bail!("source {} is not a directory", src.display());
    }
    std::fs::create_dir_all(dst)
        .with_context(|| format!("creating {}", dst.display()))?;
    for entry in WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = entry
            .path()
            .strip_prefix(src)
            .expect("walkdir paths are prefixed by root");
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("creating {}", target.display()))?;
        } else if entry.file_type().is_symlink() {
            // Materialize symlink targets as regular copies (portable snapshots).
            let link = std::fs::read_link(entry.path())
                .with_context(|| format!("reading symlink {}", entry.path().display()))?;
            if link.is_absolute() || link.components().any(|c| c == std::path::Component::ParentDir) {
                anyhow::bail!(
                    "refusing to copy escaping symlink {} -> {}",
                    entry.path().display(),
                    link.display()
                );
            }
            let real = entry.path().join(&link);
            if real.is_dir() {
                copy_dir_recursive(&real, &target)?;
            } else {
                std::fs::copy(&real, &target)
                    .with_context(|| format!("copying {}", real.display()))?;
            }
        } else {
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// State (.installer-state.json at the repo root) for `make sync`
// ---------------------------------------------------------------------------

fn state_path(repo_root: &Path) -> PathBuf {
    repo_root.join(STATE_NAME)
}

pub fn load_state(repo_root: &Path) -> State {
    std::fs::read_to_string(state_path(repo_root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_state(repo_root: &Path, state: &State) -> Result<()> {
    let raw = serde_json::to_string_pretty(state).context("serializing state")?;
    std::fs::write(state_path(repo_root), raw)
        .with_context(|| format!("writing {}", state_path(repo_root).display()))
}

pub fn scope_state_key(scope: &Scope) -> String {
    match scope {
        Scope::User => "user".to_string(),
        Scope::Project(p) => format!("project:{}", p.display()),
    }
}

/// Record a completed selection so `--sync` can replay it.
pub fn record_selection(
    repo_root: &Path,
    harnesses: &[Harness],
    scope: &Scope,
    item_names: &[String],
) -> Result<()> {
    let mut state = load_state(repo_root);
    let key = scope_state_key(scope);
    for h in harnesses {
        let sel = StateSelection {
            harness: h.key().to_string(),
            scope: key.clone(),
            items: item_names.to_vec(),
        };
        state
            .selections
            .retain(|s| !(s.harness == sel.harness && s.scope == sel.scope));
        state.selections.push(sel);
    }
    save_state(repo_root, &state)
}

/// Parse a state scope key back into a Scope.
pub fn parse_scope_key(key: &str) -> Option<Scope> {
    if key == "user" {
        Some(Scope::User)
    } else if let Some(path) = key.strip_prefix("project:") {
        Some(Scope::Project(PathBuf::from(path)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Skill, Subagent};

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn unique_suffix() -> u64 {
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn repo() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ask-install-{}-{}", std::process::id(), unique_suffix()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("skills/tdd")).unwrap();
        std::fs::write(
            d.join("skills/tdd/SKILL.md"),
            "---\nname: tdd\ndescription: test driven\n---\nBody\n",
        )
        .unwrap();
        std::fs::create_dir_all(d.join("subagents")).unwrap();
        std::fs::write(
            d.join("subagents/reviewer.md"),
            "---\nname: reviewer\ndescription: reviews code\n---\nPrompt\n",
        )
        .unwrap();
        d
    }

    fn items(repo: &Path) -> Vec<Item> {
        crate::scan::scan_valid(repo)
    }

    fn fake_dest_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ask-dest-{name}-{}-{}", std::process::id(), unique_suffix()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn installs_skill_and_agent_with_manifest() {
        let repo = repo();
        let dir = fake_dest_dir("manifest");
        let mut manifest = Manifest::new();
        let tdd = items(&repo).into_iter().find(|i| i.name() == "tdd").unwrap();
        let dest = harness::Destination {
            dir: dir.clone(),
            leaf: "tdd".into(),
        };
        std::fs::create_dir_all(&dir).unwrap();
        copy_dir_recursive(tdd.source(), &dest.full_path()).unwrap();
        manifest.insert(
            "tdd".into(),
            ManifestEntry { kind: "skill".into(), name: "tdd".into(), source: "skills/tdd".into() },
        );
        write_manifest(&dir, &manifest).unwrap();
        let read = read_manifest(&dir);
        assert_eq!(read.get("tdd").unwrap().name, "tdd");
        assert!(dir.join("tdd/SKILL.md").exists());
        assert!(dir.join(MANIFEST_NAME).exists());
    }

    #[test]
    fn foreign_file_is_skipped_unless_force() {
        let repo = repo();
        let dir = fake_dest_dir("foreign");
        // foreign skill dir already present at the destination
        let skills_dir = dir.join(".claude/skills");
        std::fs::create_dir_all(skills_dir.join("tdd")).unwrap();
        std::fs::write(skills_dir.join("tdd/SKILL.md"), "foreign").unwrap();
        // no manifest → foreign
        let skill_item = items(&repo).into_iter().find(|i| i.name() == "tdd").unwrap();
        let opts = InstallOptions::default();
        let report = install_items(&[skill_item.clone()], Harness::Claude, &Scope::Project(dir.clone()), &repo, &opts);
        assert!(matches!(report.outcomes[0], Outcome::SkippedForeign { .. }));
        // content untouched
        assert_eq!(std::fs::read_to_string(skills_dir.join("tdd/SKILL.md")).unwrap(), "foreign");

        // force overwrites
        let opts = InstallOptions { force: true, ..Default::default() };
        let report = install_items(&[skill_item], Harness::Claude, &Scope::Project(dir.clone()), &repo, &opts);
        assert!(matches!(report.outcomes[0], Outcome::Installed { .. }));
        assert!(std::fs::read_to_string(skills_dir.join("tdd/SKILL.md")).unwrap().contains("name: tdd"));
    }

    #[test]
    fn reinstall_managed_overwrites() {
        let repo = repo();
        let dir = fake_dest_dir("managed");
        let item = items(&repo).into_iter().find(|i| i.name() == "reviewer").unwrap();
        let scope = Scope::Project(dir.clone());
        let opts = InstallOptions::default();
        let r1 = install_items(&[item.clone()], Harness::Opencode, &scope, &repo, &opts);
        assert!(matches!(r1.outcomes[0], Outcome::Installed { .. }));
        let r2 = install_items(&[item], Harness::Opencode, &scope, &repo, &opts);
        assert!(matches!(r2.outcomes[0], Outcome::Installed { .. }), "managed reinstall should overwrite");
    }

    #[test]
    fn opencode_agent_has_mode_subagent() {
        let repo = repo();
        let dir = fake_dest_dir("opencode");
        let item = items(&repo).into_iter().find(|i| i.name() == "reviewer").unwrap();
        install_items(&[item], Harness::Opencode, &Scope::Project(dir.clone()), &repo, &InstallOptions::default());
        let written = std::fs::read_to_string(dir.join(".opencode/agents/reviewer.md")).unwrap();
        assert!(written.contains("mode: subagent"));
        assert!(!written.contains("name: reviewer"));
    }

    #[test]
    fn dry_run_writes_nothing() {
        let repo = repo();
        let dir = fake_dest_dir("dry");
        let item = items(&repo).into_iter().find(|i| i.name() == "tdd").unwrap();
        let report = install_items(&[item], Harness::Pi, &Scope::Project(dir.clone()), &repo, &InstallOptions { dry_run: true, ..Default::default() });
        assert!(matches!(report.outcomes[0], Outcome::Installed { .. }));
        assert!(!dir.join(".pi/skills/tdd/SKILL.md").exists());
        assert!(!dir.join(".pi/skills").join(MANIFEST_NAME).exists());
    }

    #[test]
    fn state_roundtrip() {
        let repo = repo();
        record_selection(&repo, &[Harness::Pi, Harness::Claude], &Scope::User, &["tdd".into(), "reviewer".into()]).unwrap();
        let state = load_state(&repo);
        assert_eq!(state.selections.len(), 2);
        assert_eq!(state.selections[0].items, vec!["tdd", "reviewer"]);
        assert_eq!(parse_scope_key(&state.selections[0].scope), Some(Scope::User));
    }
}
