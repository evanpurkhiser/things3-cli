use crate::common::ICONS;
use crate::ids::ThingsId;
use crate::store::{Task, ThingsStore};
use crate::ui::widgets::left_border::LeftBorderWidget;
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

fn detail_height(task: &Task) -> u16 {
    task.notes
        .as_ref()
        .map(|n| n.lines().count() as u16)
        .unwrap_or(0)
}

/// Presentational widget for a single project row, optionally with notes.
///
/// Same two-column grid as [`TaskItemWidget`]:
/// ```text
/// [id | progress-marker + markers + title + deadline]
///      [LeftBorderWidget + note lines at x=2        ]
/// ```
pub struct ProjectItemWidget<'a> {
    pub project: &'a Task,
    pub store: &'a ThingsStore,
    pub show_indicators: bool,
    pub show_staged_today_marker: bool,
    pub id_prefix_len: usize,
    pub detailed: bool,
    pub today: &'a DateTime<Utc>,
}

impl<'a> ProjectItemWidget<'a> {
    fn dim() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    fn progress_marker(&self) -> &'static str {
        if self.project.in_someday() {
            return ICONS.anytime;
        }
        let progress = self.store.project_progress(&self.project.uuid);
        let total = progress.total;
        let done = progress.done;
        if total == 0 || done == 0 {
            ICONS.progress_empty
        } else if done == total {
            ICONS.progress_full
        } else {
            let ratio = done as f32 / total as f32;
            if ratio < (1.0 / 3.0) {
                ICONS.progress_quarter
            } else if ratio < (2.0 / 3.0) {
                ICONS.progress_half
            } else {
                ICONS.progress_three_quarter
            }
        }
    }

    fn render_project_row(&self, area: Rect, buf: &mut Buffer) {
        let [marker_col, content_col] = Layout::horizontal([
            Constraint::Length(1), // progress marker
            Constraint::Fill(1),   // content
        ])
        .spacing(1)
        .areas(area);

        Span::styled(self.progress_marker(), Self::dim()).render(marker_col, buf);

        let mut spans: Vec<Span> = Vec::new();

        // Today/evening/staged marker
        if self.show_indicators {
            if self.project.evening {
                spans.push(Span::styled(ICONS.evening, Color::Blue));
                spans.push(Span::raw(" "));
            } else if self.project.is_today(self.today) {
                spans.push(Span::styled(ICONS.today, Color::Yellow));
                spans.push(Span::raw(" "));
            }
        } else if self.show_staged_today_marker && self.project.is_staged_for_today(self.today) {
            spans.push(Span::styled(ICONS.today_staged, Color::Yellow));
            spans.push(Span::raw(" "));
        }

        // Title
        if self.project.title.is_empty() {
            spans.push(Span::styled("(untitled)", Self::dim()));
        } else {
            spans.push(Span::raw(self.project.title.clone()));
        }

        // Deadline
        if let Some(deadline) = self.project.deadline {
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

    fn render_detail_block(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let children: Vec<Line> = self
            .project
            .notes
            .as_ref()
            .map(|n| {
                n.lines()
                    .map(|line| Line::from(Span::styled(line.to_owned(), Self::dim())))
                    .collect()
            })
            .unwrap_or_default();

        let h = children.len() as u16;
        LeftBorderWidget {
            children,
            total_height: h,
        }
        .render(area, buf);
    }

    pub fn height(&self) -> u16 {
        let dh = if self.detailed {
            detail_height(self.project)
        } else {
            0
        };
        1 + dh
    }
}

impl<'a> Widget for ProjectItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let id_width = self.id_prefix_len as u16;
        let detail_h = if self.detailed {
            detail_height(self.project)
        } else {
            0
        };

        let [id_col, content_col] = Layout::horizontal([
            Constraint::Length(id_width + if id_width > 0 { 1 } else { 0 }),
            Constraint::Fill(1),
        ])
        .areas(area);

        // Render short id in col 0, top row only.
        if id_width > 0 {
            let id_raw: String = self
                .project
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

        let col1 = content_col;

        if detail_h == 0 {
            self.render_project_row(Rect { height: 1, ..col1 }, buf);
        } else {
            let [project_row, detail_row] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(detail_h)]).areas(col1);

            self.render_project_row(project_row, buf);
            self.render_detail_block(detail_row, buf);
        }
    }
}

/// Render a project header line used in grouped views.
pub fn render_project_header(
    project_uuid: &ThingsId,
    store: &ThingsStore,
    id_prefix_len: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    let id_width = id_prefix_len as u16;
    let [id_col, content_col] = Layout::horizontal([
        Constraint::Length(id_width + if id_width > 0 { 1 } else { 0 }),
        Constraint::Fill(1),
    ])
    .areas(area);

    if id_width > 0 {
        let id_raw: String = project_uuid
            .to_string()
            .chars()
            .take(id_prefix_len)
            .collect();
        Span::styled(id_raw, Style::default().add_modifier(Modifier::DIM)).render(id_col, buf);
    }

    let title = store.resolve_project_title(project_uuid);
    let spans = vec![
        Span::raw(format!("{} ", ICONS.project)),
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
    ];
    Line::from(spans).render(content_col, buf);
}
