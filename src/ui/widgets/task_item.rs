use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use crate::ui::widgets::checklist::ChecklistWidget;
use crate::ui::widgets::left_border::LeftBorderWidget;
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Full height of the detail block (the LeftBorderWidget area) for this task.
/// Returns 0 if there is nothing to show.
fn detail_height(task: &Task) -> u16 {
    let note_lines = task.notes.as_ref().map(|n| n.lines().count()).unwrap_or(0);
    let checklist = task.checklist_items.len();
    let spacer = if note_lines > 0 && checklist > 0 {
        1
    } else {
        0
    };
    (note_lines + spacer + checklist) as u16
}

/// Presentational widget for a single task (todo) row, optionally with notes
/// and checklist items shown beneath it.
///
/// Layout:
/// ```text
/// Layout::horizontal([Length(id_width), Fill(1)])
///   col 0: short id text (dim), only on row 0
///   col 1: Layout::vertical([Length(1), Length(detail_height)])
///     row 0: Layout::horizontal([Length(1), Length(1), Fill(1)])
///              [▢/◼/…][ ][LineItem: markers + title + tags + deadline]
///     row 1: LeftBorderWidget + content rendered into same area
///              notes at x=2, checklist rows at x=0 (overwriting border)
/// ```
pub struct TaskItemWidget<'a> {
    pub task: &'a Task,
    pub store: &'a ThingsStore,
    /// Show project name as a suffix.
    pub show_project: bool,
    /// Show ⭑/☽ today/evening markers.
    pub show_today_markers: bool,
    /// Show ● staged-for-today marker.
    pub show_staged_today_marker: bool,
    /// Width of the shared ID column across all items in the list (0 = no IDs).
    pub id_prefix_len: usize,
    /// Whether to render notes and checklist items below the task line.
    pub detailed: bool,
    pub today: &'a DateTime<Utc>,
}

impl<'a> TaskItemWidget<'a> {
    fn dim() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    fn checkbox_str(&self) -> &'static str {
        if self.task.is_completed() {
            ICONS.task_done
        } else if self.task.is_canceled() {
            ICONS.task_canceled
        } else if self.task.in_someday() {
            ICONS.task_someday
        } else {
            ICONS.task_open
        }
    }

    /// Render the first row: [checkbox][ ][markers + title + tags + deadline]
    fn render_task_row(&self, area: Rect, buf: &mut Buffer) {
        let [checkbox_col, content_col] = Layout::horizontal([
            Constraint::Length(1), // checkbox
            Constraint::Fill(1),   // content
        ])
        .spacing(1)
        .areas(area);

        // Checkbox
        Span::styled(self.checkbox_str(), Self::dim()).render(checkbox_col, buf);

        // Content: build a Line of spans for markers + title + tags + deadline
        let mut spans: Vec<Span> = Vec::new();

        // Optional today/evening/staged marker
        if self.show_today_markers {
            if self.task.evening {
                spans.push(Span::styled(ICONS.evening, Color::Blue));
                spans.push(Span::raw(" "));
            } else if self.task.is_today(self.today) {
                spans.push(Span::styled(ICONS.today, Color::Yellow));
                spans.push(Span::raw(" "));
            }
        } else if self.show_staged_today_marker && self.task.is_staged_for_today(self.today) {
            spans.push(Span::styled(ICONS.today_staged, Color::Yellow));
            spans.push(Span::raw(" "));
        }

        // Title
        if self.task.title.is_empty() {
            spans.push(Span::styled("(untitled)", Self::dim()));
        } else {
            spans.push(Span::raw(self.task.title.clone()));
        }

        // Tags
        if !self.task.tags.is_empty() {
            let tag_names: Vec<String> = self
                .task
                .tags
                .iter()
                .map(|t| self.store.resolve_tag_title(t))
                .collect();
            spans.push(Span::styled(
                format!(" [{}]", tag_names.join(", ")),
                Self::dim(),
            ));
        }

        // Project
        if self.show_project {
            if let Some(proj) = self.store.effective_project_uuid(self.task) {
                let title = self.store.resolve_project_title(&proj);
                spans.push(Span::styled(
                    format!(" {} {}", ICONS.separator, title),
                    Self::dim(),
                ));
            }
        }

        // Deadline
        if let Some(deadline) = self.task.deadline {
            let date_str = deadline.format("%Y-%m-%d").to_string();
            let dl_style = if deadline < *self.today {
                Style::from(Color::Red)
            } else {
                Style::from(Color::Yellow)
            };
            spans.push(Span::raw(format!(" {} due by ", ICONS.deadline)));
            spans.push(Span::styled(date_str, dl_style));
        }

        Line::from(spans).render(content_col, buf);
    }

    /// Render the detail block (notes + checklist) into `area`.
    ///
    /// `area` is the detail region in the content column (below the title row).
    ///
    /// Layout per row:
    /// - Note rows: rendered inside `LeftBorderWidget` (container) at x+2
    /// - Empty spacer row: a blank `Line` inside `LeftBorderWidget`
    /// - Checklist rows: span the full width so IDs align with task IDs above
    fn render_detail_block(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let note_lines: Vec<&str> = self
            .task
            .notes
            .as_ref()
            .map(|n| n.lines().collect())
            .unwrap_or_default();

        let checklist = &self.task.checklist_items;
        let has_notes = !note_lines.is_empty();
        let has_checklist = !checklist.is_empty();

        // Build children for LeftBorderWidget: note lines + optional blank spacer.
        let mut border_children: Vec<Line> = note_lines
            .iter()
            .map(|note| Line::from(Span::styled(*note, Self::dim())))
            .collect();

        if has_notes && has_checklist {
            border_children.push(Line::default());
        }

        // The rail must extend through the checklist rows so │ is drawn there
        // too (checklist connectors ├╴/└╴ overwrite those cells).
        let rail_height = border_children.len() as u16 + checklist.len() as u16;

        if rail_height > 0 {
            LeftBorderWidget {
                children: border_children,
                total_height: rail_height,
            }
            .render(area, buf);
        }

        if has_checklist {
            let show_ids = self.id_prefix_len > 0;
            let notes_and_spacer_height =
                note_lines.len() as u16 + if has_notes && has_checklist { 1 } else { 0 };

            // Align checklist IDs under the task ID column.
            //
            // `id_prefix_len` is the task ID column width (id chars).
            // The task id col occupies `id_prefix_len + 1` cells (id + space).
            // ChecklistWidget will draw its own id col of `cl_id_col` cells.
            // If they differ, shift cl_area right by the gap so the connector
            // `├╴` always lands in the same column as the task checkbox.
            let task_id_col = if show_ids {
                self.id_prefix_len as u16 + 1
            } else {
                0
            };
            let cl_id_col = ChecklistWidget::id_col_width(checklist, show_ids);
            let x_offset = task_id_col.saturating_sub(cl_id_col);

            let cl_area = Rect {
                // Detail area lives in the content column. Shift checklist left
                // so its id column can extend into the task-id column when the
                // task-id width is larger than the checklist-id width.
                x: area.x.saturating_sub(cl_id_col) + x_offset,
                y: area.y + notes_and_spacer_height,
                width: area
                    .width
                    .saturating_add(cl_id_col)
                    .saturating_sub(x_offset),
                height: checklist.len() as u16,
            };
            ChecklistWidget {
                items: checklist,
                show_ids,
            }
            .render(cl_area, buf);
        }
    }

    /// Total height this widget needs.
    pub fn height(&self) -> u16 {
        let dh = if self.detailed {
            detail_height(self.task)
        } else {
            0
        };
        1 + dh
    }
}

impl<'a> Widget for TaskItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let id_width = self.id_prefix_len as u16;
        let detail_h = if self.detailed {
            detail_height(self.task)
        } else {
            0
        };

        // Top-level 2-column grid: [id_col | content_col]
        let [id_col, content_col] = Layout::horizontal([
            Constraint::Length(id_width + if id_width > 0 { 1 } else { 0 }), // id + space
            Constraint::Fill(1),
        ])
        .areas(area);

        // Render the short id in col 0, top row only.
        if id_width > 0 {
            let id_raw: String = self
                .task
                .uuid
                .to_string()
                .chars()
                .take(self.id_prefix_len)
                .collect();
            let id_area = Rect {
                height: 1,
                ..id_col
            };
            Span::styled(id_raw, Self::dim()).render(id_area, buf);
        }

        // Col 1: vertical split into task row + optional detail block.
        let col1 = content_col;

        if detail_h == 0 {
            // Simple case: just the task row.
            self.render_task_row(Rect { height: 1, ..col1 }, buf);
        } else {
            let [task_row, detail_row] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(detail_h)]).areas(col1);

            self.render_task_row(task_row, buf);
            self.render_detail_block(detail_row, buf);
        }
    }
}
