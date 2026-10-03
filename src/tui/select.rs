//! Generic multi-select list state shared by the harness and item screens.

#[derive(Debug, Clone)]
pub struct SelectRow {
    /// Display label.
    pub label: String,
    /// Right-aligned hint (e.g. kind or path).
    pub hint: String,
    /// Group header rows are not selectable.
    pub header: bool,
    pub selected: bool,
}

#[derive(Debug, Default)]
pub struct MultiSelect {
    pub rows: Vec<SelectRow>,
    pub cursor: usize,
}

impl MultiSelect {
    pub fn new(rows: Vec<SelectRow>) -> Self {
        let cursor = rows.iter().position(|r| !r.header).unwrap_or(0);
        Self { rows, cursor }
    }

    pub fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        Self::new(
            pairs
                .into_iter()
                .map(|(label, hint)| SelectRow { label, hint, header: false, selected: false })
                .collect(),
        )
    }

    fn next_selectable(&self, from: usize, forward: bool) -> Option<usize> {
        let mut i = from;
        loop {
            if forward {
                i = if i + 1 >= self.rows.len() { 0 } else { i + 1 };
            } else {
                i = i.checked_sub(1).unwrap_or(self.rows.len() - 1);
            }
            if i == from {
                return None;
            }
            if !self.rows[i].header {
                return Some(i);
            }
        }
    }

    pub fn move_cursor(&mut self, forward: bool) {
        if self.rows.is_empty() {
            return;
        }
        if !self.rows[self.cursor].header {
            if let Some(next) = self.next_selectable(self.cursor, forward) {
                self.cursor = next;
            }
            return;
        }
        // cursor on a header (initial state): jump to first selectable in direction
        if let Some(next) = self.next_selectable(self.cursor, forward) {
            self.cursor = next;
        }
    }

    pub fn toggle(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cursor) {
            if !row.header {
                row.selected = !row.selected;
            }
        }
    }

    pub fn select_all(&mut self, on: bool) {
        for row in &mut self.rows {
            if !row.header {
                row.selected = on;
            }
        }
    }

    pub fn selected_labels(&self) -> Vec<&str> {
        self.rows
            .iter()
            .filter(|r| !r.header && r.selected)
            .map(|r| r.label.as_str())
            .collect()
    }

    pub fn any_selected(&self) -> bool {
        self.rows.iter().any(|r| !r.header && r.selected)
    }

    /// Render rows into ratatui `Line`s for a centered popup.
    pub fn render_lines(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};

        self.rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.header {
                    return Line::from(Span::styled(
                        row.label.clone(),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ));
                }
                let checkbox = if row.selected { "[x] " } else { "[ ] " };
                let mut spans = vec![Span::styled(
                    format!("{checkbox}{}", row.label),
                    if i == self.cursor {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                )];
                if !row.hint.is_empty() {
                    spans.push(Span::styled(
                        format!("  — {}", row.hint),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                Line::from(spans)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MultiSelect {
        MultiSelect::new(vec![
            SelectRow { label: "Skills".into(), hint: String::new(), header: true, selected: false },
            SelectRow { label: "tdd".into(), hint: "skill".into(), header: false, selected: false },
            SelectRow { label: "Subagents".into(), hint: String::new(), header: true, selected: false },
            SelectRow { label: "reviewer".into(), hint: "subagent".into(), header: false, selected: true },
        ])
    }

    #[test]
    fn cursor_starts_on_first_selectable() {
        let ms = sample();
        assert_eq!(ms.rows[ms.cursor].label, "tdd");
        assert!(!ms.rows[ms.cursor].header);
    }

    #[test]
    fn movement_skips_headers_and_wraps() {
        let mut ms = sample();
        ms.move_cursor(true); // tdd -> reviewer
        assert_eq!(ms.rows[ms.cursor].label, "reviewer");
        ms.move_cursor(true); // wraps to tdd
        assert_eq!(ms.rows[ms.cursor].label, "tdd");
        ms.move_cursor(false); // wraps back to reviewer
        assert_eq!(ms.rows[ms.cursor].label, "reviewer");
    }

    #[test]
    fn toggle_and_select_all() {
        let mut ms = sample();
        ms.cursor = 1;
        ms.toggle();
        assert!(ms.rows[1].selected);
        ms.select_all(true);
        assert_eq!(ms.selected_labels().len(), 2);
        ms.select_all(false);
        assert!(ms.selected_labels().is_empty());
        assert!(!ms.any_selected());
    }

    #[test]
    fn toggle_ignores_headers() {
        let mut ms = sample();
        ms.cursor = 0;
        ms.toggle();
        assert!(!ms.rows[0].selected);
    }
}
