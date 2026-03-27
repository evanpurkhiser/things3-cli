use crate::ids::ThingsId;
use crate::ui::widgets::area_header::AreaHeaderWidget;
use crate::ui::widgets::project_header::ProjectHeaderWidget;
use crate::ui::widgets::tasks::TasksWidget;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

pub enum TaskGroupHeader {
    Area {
        area_uuid: ThingsId,
        title: String,
        id_prefix_len: usize,
    },
    Project {
        project_uuid: ThingsId,
        title: String,
        id_prefix_len: usize,
    },
}

pub struct TaskGroupWidget<'a> {
    pub header: Option<TaskGroupHeader>,
    pub tasks: TasksWidget<'a>,
    pub indent_under_header: u16,
}

impl<'a> TaskGroupWidget<'a> {
    pub fn height(&self) -> u16 {
        let header_h = if self.header.is_some() { 1 } else { 0 };
        header_h + self.tasks.height()
    }
}

impl<'a> Widget for TaskGroupWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let has_header = self.header.is_some();
        let mut y = area.y;

        if let Some(header) = self.header {
            match header {
                TaskGroupHeader::Area {
                    area_uuid,
                    title,
                    id_prefix_len,
                } => AreaHeaderWidget {
                    area_uuid: &area_uuid,
                    title: &title,
                    id_prefix_len,
                }
                .render(
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                ),
                TaskGroupHeader::Project {
                    project_uuid,
                    title,
                    id_prefix_len,
                } => ProjectHeaderWidget {
                    project_uuid: &project_uuid,
                    title: &title,
                    id_prefix_len,
                }
                .render(
                    Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                ),
            }
            y = y.saturating_add(1);
        }

        let body_x = if has_header {
            area.x.saturating_add(self.indent_under_header)
        } else {
            area.x
        };
        let body_width = if has_header {
            area.width.saturating_sub(self.indent_under_header)
        } else {
            area.width
        };
        let body_h = self.tasks.height();

        self.tasks.render(
            Rect {
                x: body_x,
                y,
                width: body_width,
                height: body_h,
            },
            buf,
        );
    }
}
