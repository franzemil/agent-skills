//! Project-context detection: are we inside a "project" that could receive
//! Local (project-level) installs, and if so, which directory?

use std::path::{Path, PathBuf};

/// Files that mark a directory as a project root (besides `.git`).
const MARKERS: &[&str] = &[
    "package.json",
    "pyproject.toml",
    "Cargo.toml",
    "go.mod",
    "composer.json",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "deno.json",
    "mix.exs",
];

/// Result of scanning a start directory for project context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    /// Nearest enclosing project root, if any.
    pub project_root: Option<PathBuf>,
    /// True when `start` is inside the agent-skills repo itself.
    pub in_installer_repo: bool,
}

/// Walk up from `start` looking for the nearest project root: a `.git` dir or a
/// marker file. Stops at the filesystem root. Directories named `target` are
/// skipped as candidates (build output inside the installer repo).
pub fn detect_project(start: &Path, installer_repo: &Path) -> ProjectContext {
    let mut dir: Option<&Path> = Some(start);
    while let Some(d) = dir {
        let has_git = d.join(".git").exists();
        let has_marker = MARKERS.iter().any(|m| d.join(m).is_file());
        if has_git || has_marker {
            // The installer repo itself only counts as a project when the user
            // ran the binary from somewhere else and we walked into it.
            let is_repo = starts_with(d, installer_repo);
            return ProjectContext {
                project_root: Some(d.to_path_buf()),
                in_installer_repo: is_repo,
            };
        }
        dir = d.parent();
    }
    ProjectContext {
        project_root: None,
        in_installer_repo: false,
    }
}

fn starts_with(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}

/// Should the TUI offer a "Local" scope option for this context?
/// Yes when a project root was found that is not the installer repo itself.
pub fn local_candidate(ctx: &ProjectContext) -> Option<&Path> {
    match (&ctx.project_root, ctx.in_installer_repo) {
        (Some(root), false) => Some(root),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmpdir(name: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir()
            .join(format!("agent-skills-test-{name}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn finds_git_root_from_nested_dir() {
        let root = tmpdir("gitroot");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        let ctx = detect_project(&root.join("a/b"), Path::new("/nonexistent"));
        assert_eq!(ctx.project_root.as_deref(), Some(root.as_path()));
        assert!(!ctx.in_installer_repo);
        assert!(local_candidate(&ctx).is_some());
    }

    #[test]
    fn finds_marker_root() {
        let root = tmpdir("marker");
        std::fs::write(root.join("go.mod"), "module x").unwrap();
        let ctx = detect_project(&root, Path::new("/nonexistent"));
        assert_eq!(ctx.project_root.as_deref(), Some(root.as_path()));
    }

    #[test]
    fn no_root_means_no_local() {
        let root = tmpdir("bare");
        let ctx = detect_project(&root, Path::new("/nonexistent"));
        assert!(ctx.project_root.is_none());
        assert!(local_candidate(&ctx).is_none());
    }

    #[test]
    fn installer_repo_is_not_a_local_candidate() {
        let repo = tmpdir("repo");
        std::fs::create_dir_all(repo.join("skills")).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let ctx = detect_project(&repo, &repo);
        assert!(ctx.in_installer_repo);
        assert!(local_candidate(&ctx).is_none());
    }

    #[test]
    fn nested_project_inside_repo_counts() {
        // e.g. a vendored example project inside the installer repo
        let repo = tmpdir("repo2");
        let nested = repo.join("examples/proj");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("package.json"), "{}").unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let ctx = detect_project(&nested, &repo);
        assert!(ctx.in_installer_repo);
        assert!(local_candidate(&ctx).is_none()); // conservative v1: skip nested-in-repo
    }
}
