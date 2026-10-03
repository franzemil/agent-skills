//! Scope picker and confirm/result screen rendering.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::model::Scope;

/// Options shown on the scope screen.
pub struct ScopeOptions {
    pub user: Scope,
    pub local: Option<Scope>,
}

impl ScopeOptions {
    pub fn ordered(&self) -> Vec<&Scope> {
        let mut v = vec![&self.user];
        if let Some(local) = &self.local {
            v.push(local);
        }
        v
    }
}

pub fn scope_lines(opts: &ScopeOptions, cursor: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "Install scope",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    ))];
    for (i, scope) in opts.ordered().iter().enumerate() {
        let arrow = if i == cursor { "> " } else { "  " };
        let style = if i == cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{arrow}{}", scope.label()),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑/↓ move · Enter confirm · Esc back",
        Style::default().fg(Color::DarkGray),
    )));
    lines
}

pub fn scope_paragraph(opts: &ScopeOptions, cursor: usize) -> Paragraph<'static> {
    Paragraph::new(scope_lines(opts, cursor)).wrap(Wrap { trim: false })
}

/// Confirm screen: summary of what is about to happen.
pub struct ConfirmPlan {
    pub harness_labels: Vec<String>,
    pub scope_label: String,
    pub item_names: Vec<String>,
    pub dry_run: bool,
}

pub fn confirm_paragraph(plan: &ConfirmPlan) -> Paragraph<'static> {
    let mut lines = vec![Line::from(Span::styled(
        "Ready to install",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )), Line::from("")];
    lines.push(Line::from(format!(
        "Harnesses : {}",
        plan.harness_labels.join(", ")
    )));
    lines.push(Line::from(format!("Scope     : {}", plan.scope_label)));
    lines.push(Line::from(format!("Items     : {}", plan.item_names.len())));
    for name in &plan.item_names {
        lines.push(Line::from(Span::styled(
            format!("  • {name}"),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));
    if plan.dry_run {
        lines.push(Line::from(Span::styled(
            "DRY RUN — nothing will be written",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Enter install · Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));
    Paragraph::new(lines).wrap(Wrap { trim: false })
}

/// Result screen after installation completes.
#[derive(Debug, Clone, Default)]
pub struct InstallSummary {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub dry_run: bool,
}

impl InstallSummary {
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty() && self.skipped.is_empty() && self.failed.is_empty()
    }
}

pub fn result_paragraph(summary: &InstallSummary) -> Paragraph<'static> {
    let mut lines = vec![Line::from(Span::styled(
        if summary.dry_run { "Dry run complete" } else { "Installation complete" },
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )), Line::from("")];
    for path in &summary.installed {
        lines.push(Line::from(Span::styled(
            format!("✓ {path}"),
            Style::default().fg(Color::Green),
        )));
    }
    for path in &summary.skipped {
        lines.push(Line::from(Span::styled(
            format!("⚠ skipped (foreign file): {path}"),
            Style::default().fg(Color::Yellow),
        )));
    }
    for (path, err) in &summary.failed {
        lines.push(Line::from(Span::styled(
            format!("✗ {path}: {err}"),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "q / Esc quit",
        Style::default().fg(Color::DarkGray),
    )));
    Paragraph::new(lines).wrap(Wrap { trim: true })
}
