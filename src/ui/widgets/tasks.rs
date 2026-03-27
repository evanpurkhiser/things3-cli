use crate::store::{Task, ThingsStore};
use crate::ui::widgets::project_item::ProjectItemWidget;
use crate::ui::widgets::task_item::TaskItemWidget;
use chrono::{DateTime, Utc};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

enum RowWidget<'a> {
    Task(TaskItemWidget<'a>),
    Project(ProjectItemWidget<'a>),
}

impl<'a> RowWidget<'a> {
    fn height(&self) -> u16 {
        match self {
            Self::Task(w) => w.height(),
            Self::Project(w) => w.height(),
        }
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Task(w) => w.render(area, buf),
            Self::Project(w) => w.render(area, buf),
        }
    }
}

pub struct TasksWidget<'a> {
    pub items: &'a [&'a Task],
    pub store: &'a ThingsStore,
    pub today: &'a DateTime<Utc>,
    pub id_prefix_len: usize,
    pub detailed: bool,
    pub show_project: bool,
    pub show_area: bool,
    pub show_today_markers: bool,
    pub show_staged_today_marker: bool,
}

impl<'a> TasksWidget<'a> {
    fn row_widget(&self, item: &'a Task) -> RowWidget<'a> {
        if item.is_project() {
            RowWidget::Project(ProjectItemWidget {
                project: item,
                store: self.store,
                show_indicators: false,
                show_staged_today_marker: self.show_staged_today_marker,
                show_area: self.show_area,
                id_prefix_len: self.id_prefix_len,
                detailed: self.detailed,
                today: self.today,
            })
        } else {
            RowWidget::Task(TaskItemWidget {
                task: item,
                store: self.store,
                show_project: self.show_project,
                show_area: self.show_area,
                show_today_markers: self.show_today_markers,
                show_staged_today_marker: self.show_staged_today_marker,
                id_prefix_len: self.id_prefix_len,
                detailed: self.detailed,
                today: self.today,
            })
        }
    }

    pub fn height(&self) -> u16 {
        self.items.iter().fold(0u16, |acc, item| {
            acc.saturating_add(self.row_widget(item).height())
        })
    }
}

impl<'a> Widget for TasksWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.items.is_empty() || area.height == 0 {
            return;
        }

        let mut y = area.y;
        for item in self.items {
            let widget = self.row_widget(item);
            let row_h = widget.height();
            widget.render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: row_h,
                },
                buf,
            );
            y = y.saturating_add(row_h);
        }
    }
}
