//! agent-skills — cross-harness skill/subagent installer.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use agent_skills::detect;
use agent_skills::install::{self, InstallOptions};
use agent_skills::model::{Harness, Item, Scope, ALL_HARNESSES};
use agent_skills::scan;
use agent_skills::tui::{self, WizardConfig, WizardOutcome};

#[derive(Debug, Parser)]
#[command(
    name = "agent-skills",
    version,
    about = "Install generic skills & subagents into pi, Claude Code or opencode"
)]
struct Cli {
    /// Preselect harness(es): pi, claude, opencode, all (comma separated)
    #[arg(long, value_delimiter = ',', value_parser = parse_harness_arg)]
    harness: Option<Vec<String>>,

    /// Install scope: auto (detect + ask), user or project
    #[arg(long, default_value = "auto")]
    scope: String,

    /// Explicit selection by item name or path (comma separated)
    #[arg(long, value_delimiter = ',')]
    items: Option<Vec<String>>,

    /// Select every item (skip the item screen)
    #[arg(long, default_value = None)]
    all_items: bool,

    /// Re-apply the last selection per harness from .installer-state.json
    #[arg(long)]
    sync: bool,

    /// List all discovered skills + subagents and exit
    #[arg(long)]
    list: bool,

    /// Validate frontmatter and exit (non-zero on problems)
    #[arg(long)]
    validate: bool,

    /// Show what would be done, write nothing
    #[arg(long)]
    dry_run: bool,

    /// Non-interactive: no TUI (requires --all-items or --items)
    #[arg(long)]
    no_tui: bool,

    /// Overwrite unmanaged destination files
    #[arg(long)]
    force: bool,
}

fn parse_harness_arg(s: &str) -> Result<String, String> {
    let lowered = s.to_ascii_lowercase();
    match lowered.as_str() {
        "all" | "pi" | "pidev" | "claude" | "claude-code" | "opencode" => Ok(lowered),
        other => Err(format!(
            "unknown harness '{other}' (expected pi, claude, opencode or all)"
        )),
    }
}

fn resolve_harnesses(spec: &[String]) -> Result<Vec<Harness>> {
    let mut out = Vec::new();
    for s in spec {
        if s == "all" {
            out.extend(ALL_HARNESSES);
        } else {
            out.push(Harness::parse(s)?);
        }
    }
    Ok(out)
}

fn resolve_scope(cli: &Cli, local_root: Option<PathBuf>) -> Result<(Option<Scope>, bool)> {
    // Returns (preselected scope, ask_at_runtime)
    match cli.scope.as_str() {
        "user" => Ok((Some(Scope::User), false)),
        "project" => {
            let root = local_root.clone().context(
                "--scope project given but no project detected around the current directory",
            )?;
            Ok((Some(Scope::Project(root)), false))
        }
        "auto" => Ok((None, true)),
        other => Err(anyhow::anyhow!(
            "unknown scope '{other}' (expected auto, user or project)"
        )),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("getting current directory")?;
    let repo_root = scan::find_repo_root(&cwd)
        .with_context(|| "run from inside the agent-skills repo (or any subdirectory)")?;

    // --list / --validate / --sync operate on the repo directly
    if cli.list {
        for item in scan::scan_valid(&repo_root) {
            println!("{:<10} {:<20} {}", item.kind(), item.name(), item.description());
        }
        return Ok(ExitCode::SUCCESS);
    }
    if cli.validate {
        let problems = scan::validate_repo(&repo_root);
        if problems.is_empty() {
            println!("agent-skills: all items valid");
            return Ok(ExitCode::SUCCESS);
        }
        for p in &problems {
            eprintln!("invalid: {p}");
        }
        eprintln!("{} problem(s) found", problems.len());
        return Ok(ExitCode::FAILURE);
    }

    let items = scan::scan_valid(&repo_root);

    if cli.sync {
        return run_sync(&cli, &repo_root, items);
    }

    // Project detection for the scope screen
    let ctx = detect::detect_project(&cwd, &repo_root);
    let local_root = detect::local_candidate(&ctx).map(PathBuf::from);

    let harnesses = match &cli.harness {
        Some(spec) => resolve_harnesses(spec)?,
        None => Vec::new(), // wizard asks
    };
    let (preselected_scope, _ask) = resolve_scope(&cli, local_root.clone())?;

    let opts = InstallOptions { force: cli.force, dry_run: cli.dry_run };

    // Non-interactive path
    if cli.no_tui {
        if cli.items.is_none() && !cli.all_items {
            anyhow::bail!("--no-tui requires --all-items or --items <names>");
        }
        let selected: Vec<Item> = if cli.all_items {
            items.clone()
        } else {
            pick_items(&items, cli.items.as_deref().unwrap_or_default())?
        };
        if selected.is_empty() {
            anyhow::bail!("no matching items");
        }
        if harnesses.is_empty() {
            anyhow::bail!("--no-tui requires --harness");
        }
        let scope = preselected_scope.unwrap_or(Scope::User);
        let mut failed = 0usize;
        for h in &harnesses {
            let report = install::install_items(&selected, *h, &scope, &repo_root, &opts);
            println!("[{}] {}", h.key(), report.summary());
            failed += report.outcomes.iter().filter(|o| matches!(o, install::Outcome::Failed { .. })).count();
        }
        if !cli.dry_run {
            let names = selected.iter().map(|i| i.name().to_string()).collect::<Vec<_>>();
            install::record_selection(&repo_root, &harnesses, &scope, &names)?;
        }
        return Ok(if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE });
    }

    // Interactive wizard
    let cfg = WizardConfig {
        repo_root,
        items,
        preselected_harnesses: harnesses,
        preselected_scope,
        all_items: cli.all_items,
        items_filter: cli.items.clone(),
        dry_run: cli.dry_run,
        force: cli.force,
        local_root,
    };
    match tui::run(cfg)? {
        WizardOutcome::Installed { summary, .. } => {
            let ok = summary.failed.is_empty();
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        WizardOutcome::Cancelled => {
            println!("cancelled");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn pick_items(items: &[Item], wanted: &[String]) -> Result<Vec<Item>> {
    let mut out = Vec::new();
    for w in wanted {
        let w = w.trim();
        let found = items
            .iter()
            .find(|i| {
                i.name() == w
                    || i.source().file_name().map(|f| f == w).unwrap_or(false)
                    || i.source().ends_with(w)
            })
            .cloned();
        match found {
            Some(item) => out.push(item),
            None => anyhow::bail!("no item matches '{w}'"),
        }
    }
    Ok(out)
}

fn run_sync(cli: &Cli, repo_root: &PathBuf, items: Vec<Item>) -> Result<ExitCode> {
    let state = install::load_state(repo_root);
    if state.selections.is_empty() {
        eprintln!("nothing to sync: no previous installation recorded");
        return Ok(ExitCode::SUCCESS);
    }
    let opts = InstallOptions { force: cli.force, dry_run: cli.dry_run };
    let mut failed = 0usize;
    for sel in &state.selections {
        let Some(h) = Harness::parse(&sel.harness).ok() else {
            eprintln!("warning: unknown harness '{}' in state, skipping", sel.harness);
            continue;
        };
        let Some(scope) = install::parse_scope_key(&sel.scope) else {
            eprintln!("warning: unparsable scope '{}' in state, skipping", sel.scope);
            continue;
        };
        let selected = pick_items(&items, &sel.items).unwrap_or_default();
        if selected.is_empty() {
            eprintln!("warning: [{}] no matching items, skipping", sel.harness);
            continue;
        }
        let report = install::install_items(&selected, h, &scope, repo_root, &opts);
        println!("[{} / {}] {}", sel.harness, sel.scope, report.summary());
        failed += report
            .outcomes
            .iter()
            .filter(|o| matches!(o, install::Outcome::Failed { .. }))
            .count();
    }
    Ok(if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
