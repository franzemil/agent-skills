//! TUI wizard: harness → items → scope → confirm → (install) → result.

pub mod confirm;
pub mod select;

use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Terminal;

use crate::install::{self, InstallOptions};
use confirm::InstallSummary;

type Tui = Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;
use crate::model::{Harness, Item, Scope, ALL_HARNESSES};
use select::{MultiSelect, SelectRow};

/// Static configuration the wizard runs with.
pub struct WizardConfig {
    pub repo_root: PathBuf,
    pub items: Vec<Item>,
    /// Preselected harnesses (from --harness). Empty → show the harness screen.
    pub preselected_harnesses: Vec<Harness>,
    /// Preselected scope (from --scope). Skips the scope screen.
    pub preselected_scope: Option<Scope>,
    /// Select every item without showing the item screen.
    pub all_items: bool,
    /// Preselect by names (from --items).
    pub items_filter: Option<Vec<String>>,
    pub dry_run: bool,
    pub force: bool,
    /// Detected local project root (offers the Local scope option).
    pub local_root: Option<PathBuf>,
}

pub enum WizardOutcome {
    Installed {
        summary: InstallSummary,
        harnesses: Vec<Harness>,
        scope: Scope,
        item_names: Vec<String>,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Harness,
    Items,
    Scope,
    Confirm,
}

struct App {
    screen: Screen,
    came_from_harness_screen: bool,
    harness_select: MultiSelect,
    item_select: MultiSelect,
    scope_opts: confirm::ScopeOptions,
    scope_cursor: usize,
    plan: confirm::ConfirmPlan,
    result: InstallSummary,
    chosen_harnesses: Vec<Harness>,
    chosen_scope: Scope,
    chosen_items: Vec<String>,
}

fn harness_rows(preselected: &[Harness]) -> Vec<SelectRow> {
    let mut rows = vec![SelectRow {
        label: "Harnesses".into(),
        hint: String::new(),
        header: true,
        selected: false,
    }];
    for h in ALL_HARNESSES {
        rows.push(SelectRow {
            label: h.label().to_string(),
            hint: String::new(),
            header: false,
            selected: preselected.contains(&h),
        });
    }
    rows
}

fn item_rows(items: &[Item], preselected: &[String]) -> Vec<SelectRow> {
    let mut rows = Vec::new();
    let skills: Vec<_> = items.iter().filter(|i| matches!(i, Item::Skill(_))).collect();
    let agents: Vec<_> = items.iter().filter(|i| matches!(i, Item::Subagent(_))).collect();
    if !skills.is_empty() {
        rows.push(SelectRow {
            label: "Skills".into(),
            hint: format!("({})", skills.len()),
            header: true,
            selected: false,
        });
        for item in skills {
            rows.push(SelectRow {
                label: item.name().to_string(),
                hint: item.description().chars().take(60).collect(),
                header: false,
                selected: preselected.iter().any(|p| p == item.name()),
            });
        }
    }
    if !agents.is_empty() {
        rows.push(SelectRow {
            label: "Subagents".into(),
            hint: format!("({})", agents.len()),
            header: true,
            selected: false,
        });
        for item in agents {
            rows.push(SelectRow {
                label: item.name().to_string(),
                hint: item.description().chars().take(60).collect(),
                header: false,
                selected: preselected.iter().any(|p| p == item.name()),
            });
        }
    }
    rows
}

fn center(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r)[1];
    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical)[1];
    horizontal
}

impl App {
    fn selected_harnesses(&self) -> Vec<Harness> {
        ALL_HARNESSES
            .iter()
            .copied()
            .filter(|h| {
                self.harness_select
                    .rows
                    .iter()
                    .any(|r| r.label == h.label() && r.selected)
            })
            .collect()
    }

    fn selected_items(&self, items: &[Item]) -> Vec<Item> {
        items
            .iter()
            .filter(|i| {
                self.item_select
                    .rows
                    .iter()
                    .any(|r| r.label == i.name() && r.selected)
            })
            .cloned()
            .collect()
    }

    fn build_plan(&self) -> confirm::ConfirmPlan {
        confirm::ConfirmPlan {
            harness_labels: self.chosen_harnesses.iter().map(|h| h.label().to_string()).collect(),
            scope_label: self.chosen_scope.label(),
            item_names: self.chosen_items.clone(),
            dry_run: false,
        }
    }
}

/// Run the wizard. Handles terminal setup/teardown and the install step
/// (performed with the terminal restored so errors print normally).
pub fn run(cfg: WizardConfig) -> Result<WizardOutcome> {
    if cfg.items.is_empty() {
        anyhow::bail!("no installable items found under {}", cfg.repo_root.display());
    }

    let scope_opts = confirm::ScopeOptions {
        user: Scope::User,
        local: cfg.local_root.clone().map(Scope::Project),
    };
    let effective_scope = cfg.preselected_scope.clone().unwrap_or(Scope::User);

    let mut app = App {
        screen: Screen::Harness,
        came_from_harness_screen: cfg.preselected_harnesses.is_empty(),
        harness_select: MultiSelect::new(harness_rows(&cfg.preselected_harnesses)),
        item_select: MultiSelect::new(item_rows(
            &cfg.items,
            cfg.items_filter.as_deref().unwrap_or_default(),
        )),
        scope_opts,
        scope_cursor: 0,
        plan: confirm::ConfirmPlan {
            harness_labels: vec![],
            scope_label: String::new(),
            item_names: vec![],
            dry_run: cfg.dry_run,
        },
        result: InstallSummary::default(),
        chosen_harnesses: cfg.preselected_harnesses.clone(),
        chosen_scope: effective_scope,
        chosen_items: vec![],
    };

    // Skip screens when everything is preselected.
    if !cfg.preselected_harnesses.is_empty() {
        app.screen = Screen::Items;
    } else {
        app.screen = Screen::Harness;
    }
    if cfg.all_items {
        app.item_select.select_all(true);
    }

    // Fast path: nothing left to choose (harness + scope + items all given).
    if !cfg.preselected_harnesses.is_empty() && cfg.preselected_scope.is_some() && cfg.all_items {
        app.chosen_harnesses = cfg.preselected_harnesses.clone();
        app.chosen_items = app.selected_items(&cfg.items).iter().map(|i| i.name().to_string()).collect();
        return install_and_show_result(&mut app, &cfg);
    }

    let mut terminal = setup_terminal().context("initializing terminal")?;
    let flow = event_loop(&mut terminal, &mut app, &cfg);
    restore_terminal(&mut terminal)?;

    match flow {
        Flow::Install => install_and_show_result(&mut app, &cfg),
        Flow::Cancel => Ok(WizardOutcome::Cancelled),
    }
}

enum Flow {
    Install,
    Cancel,
}

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("entering alternate screen")?;
    Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))
        .context("creating terminal")
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode().context("disabling raw mode")?;
    io::stdout()
        .execute(LeaveAlternateScreen)
        .context("leaving alternate screen")?;
    terminal.show_cursor().context("showing cursor")?;
    Ok(())
}

fn event_loop(terminal: &mut Tui, app: &mut App, cfg: &WizardConfig) -> Flow {
    loop {
        let _ = terminal.draw(|frame| draw(frame, app, cfg));
        if !crossterm::event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = crossterm::event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Flow::Cancel;
        }
        match app.screen {
            Screen::Harness => match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.harness_select.move_cursor(false),
                KeyCode::Down | KeyCode::Char('j') => app.harness_select.move_cursor(true),
                KeyCode::Char(' ') => app.harness_select.toggle(),
                KeyCode::Char('a') => app.harness_select.select_all(true),
                KeyCode::Char('n') => app.harness_select.select_all(false),
                KeyCode::Enter => {
                    if app.harness_select.any_selected() {
                        app.chosen_harnesses = app.selected_harnesses();
                        app.screen = Screen::Items;
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => return Flow::Cancel,
                _ => {}
            },
            Screen::Items => match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.item_select.move_cursor(false),
                KeyCode::Down | KeyCode::Char('j') => app.item_select.move_cursor(true),
                KeyCode::Char(' ') => app.item_select.toggle(),
                KeyCode::Char('a') => app.item_select.select_all(true),
                KeyCode::Char('n') => app.item_select.select_all(false),
                KeyCode::Enter => {
                    if app.item_select.any_selected() {
                        app.screen = Screen::Scope;
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    if app.came_from_harness_screen {
                        app.screen = Screen::Harness;
                    } else {
                        return Flow::Cancel;
                    }
                }
                _ => {}
            },
            Screen::Scope => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    app.scope_cursor = app.scope_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = app.scope_opts.ordered().len().saturating_sub(1);
                    if app.scope_cursor < max {
                        app.scope_cursor += 1;
                    }
                }
                KeyCode::Enter => {
                    let scopes = app.scope_opts.ordered();
                    if let Some(scope) = scopes.get(app.scope_cursor) {
                        app.chosen_scope = (*scope).clone();
                        let items = app.selected_items(&cfg.items);
                        app.chosen_items = items.iter().map(|i| i.name().to_string()).collect();
                        app.plan = app.build_plan();
                        app.screen = Screen::Confirm;
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => app.screen = Screen::Items,
                _ => {}
            },
            Screen::Confirm => match key.code {
                KeyCode::Enter => return Flow::Install,
                KeyCode::Esc | KeyCode::Char('q') => app.screen = Screen::Scope,
                _ => {}
            },
        }
    }
}

/// Perform the installation (terminal already restored) then show a result screen.
fn install_and_show_result(app: &mut App, cfg: &WizardConfig) -> Result<WizardOutcome> {
    if app.result.is_empty() {
        let items = app.selected_items(&cfg.items);
        let opts = InstallOptions { force: cfg.force, dry_run: cfg.dry_run };
        let mut summary = InstallSummary::default();
        for h in &app.chosen_harnesses {
            let report = install::install_items(&items, *h, &app.chosen_scope, &cfg.repo_root, &opts);
            for outcome in &report.outcomes {
                match outcome {
                    install::Outcome::Installed { dest } => {
                        summary.installed.push(dest.display().to_string())
                    }
                    install::Outcome::SkippedForeign { dest } => {
                        summary.skipped.push(dest.display().to_string())
                    }
                    install::Outcome::Failed { dest, error } => {
                        summary.failed.push((dest.display().to_string(), error.clone()))
                    }
                }
            }
        }
        summary.dry_run = cfg.dry_run;
        app.result = summary;

        if !cfg.dry_run && !items.is_empty() {
            let names = items.iter().map(|i| i.name().to_string()).collect::<Vec<_>>();
            install::record_selection(
                &cfg.repo_root,
                &app.chosen_harnesses,
                &app.chosen_scope,
                &names,
            )
            .ok(); // best effort — sync just loses history on failure
        }
    }

    // Show the result screen (best effort; fall back to plain printing).
    let show = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let mut terminal = setup_terminal()?;
        loop {
            let _ = terminal.draw(|frame| {
                let area = center(70, 60, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    confirm::result_paragraph(&app.result)
                        .block(Block::default().borders(Borders::ALL).title(" Result ")),
                    area,
                );
            });
            if crossterm::event::poll(std::time::Duration::from_millis(250))? {
                if let Event::Key(key) = crossterm::event::read()? {
                    if key.kind == KeyEventKind::Press
                        && matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter)
                    {
                        break;
                    }
                }
            }
        }
        restore_terminal(&mut terminal)?;
        Ok(())
    }));
    if show.is_err() {
        eprintln!("(could not show result screen)");
    }

    // Plain summary for scrollback.
    println!(
        "agent-skills: {}",
        if app.result.failed.is_empty() { "done" } else { "completed with errors" }
    );

    Ok(WizardOutcome::Installed {
        summary: app.result.clone(),
        harnesses: app.chosen_harnesses.clone(),
        scope: app.chosen_scope.clone(),
        item_names: app.chosen_items.clone(),
    })
}

fn draw(frame: &mut ratatui::Frame, app: &App, cfg: &WizardConfig) {
    let area = center(80, 70, frame.area());
    frame.render_widget(Clear, area);
    let (title, paragraph): (&str, Paragraph) = match app.screen {
        Screen::Harness => {
            let mut lines = app.harness_select.render_lines();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ move · space toggle · a all · n none · Enter confirm · Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));
            (
                " Select harnesses ",
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(Span::styled(
                            " Select harnesses ",
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        )),
                ),
            )
        }
        Screen::Items => {
            let mut lines = app.item_select.render_lines();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ move · space toggle · a all · n none · Enter confirm · Esc back",
                Style::default().fg(Color::DarkGray),
            )));
            (
                " Select items ",
                Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(
                    Span::styled(
                        " Select skills & subagents ",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                )),
            )
        }
        Screen::Scope => (
            " Install scope ",
            confirm::scope_paragraph(&app.scope_opts, app.scope_cursor).block(
                Block::default().borders(Borders::ALL).title(Span::styled(
                    " Install scope ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )),
            ),
        ),
        Screen::Confirm => (
            " Confirm ",
            confirm::confirm_paragraph(&app.plan).block(Block::default().borders(Borders::ALL).title(
                Span::styled(
                    " Confirm ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
            )),
        ),
    };
    let _ = title;
    let _ = cfg;
    frame.render_widget(paragraph, area);
}
