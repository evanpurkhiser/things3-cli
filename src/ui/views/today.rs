use crate::common::ICONS;
use crate::ids::ThingsId;
use crate::store::{Task, ThingsStore};
use crate::ui::widgets::task_group::{TaskGroupHeader, TaskGroupWidget};
use crate::ui::widgets::tasks::TasksWidget;
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::collections::HashMap;

#[derive(Default)]
struct AreaGroup<'a> {
    tasks: Vec<&'a Task>,
}

#[derive(Default)]
struct GroupedSection<'a> {
    unscoped: Vec<&'a Task>,
    project_only: Vec<(ThingsId, Vec<&'a Task>)>,
    by_area: Vec<(ThingsId, AreaGroup<'a>)>,
}

pub struct TodayView<'a> {
    pub store: &'a ThingsStore,
    pub today: &'a DateTime<Utc>,
    pub items: Vec<Task>,
    pub id_prefix_len: usize,
    pub detailed: bool,
}

impl<'a> TodayView<'a> {
    fn dim() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn project_count(&self) -> usize {
        self.items.iter().filter(|task| task.is_project()).count()
    }

    fn task_count(&self) -> usize {
        self.items.iter().filter(|task| !task.is_project()).count()
    }

    fn has_regular(&self) -> bool {
        self.items.iter().any(|task| !task.evening)
    }

    fn has_evening(&self) -> bool {
        self.items.iter().any(|task| task.evening)
    }

    fn header_text(&self) -> String {
        let task_count = self.task_count();
        let project_count = self.project_count();
        if project_count > 0 {
            let label = if project_count == 1 {
                "project"
            } else {
                "projects"
            };
            format!(
                "{} Today  ({} tasks, {} {})",
                ICONS.today, task_count, project_count, label
            )
        } else {
            format!("{} Today  ({} tasks)", ICONS.today, task_count)
        }
    }

    fn tasks_widget<'b>(
        &'a self,
        items: &'b [&'a Task],
        show_project: bool,
        show_area: bool,
    ) -> TasksWidget<'a>
    where
        'b: 'a,
    {
        TasksWidget {
            items,
            store: self.store,
            today: self.today,
            id_prefix_len: self.id_prefix_len,
            detailed: self.detailed,
            show_project,
            show_area,
            show_today_markers: false,
            show_staged_today_marker: true,
        }
    }

    fn group_regular_items(&'a self) -> GroupedSection<'a> {
        let mut grouped = GroupedSection::default();
        let mut project_only_pos: HashMap<ThingsId, usize> = HashMap::new();
        let mut by_area_pos: HashMap<ThingsId, usize> = HashMap::new();

        for task in self.items.iter().filter(|task| !task.evening) {
            if task.is_project() {
                if let Some(area_uuid) = self.store.effective_area_uuid(task) {
                    let area_idx = if let Some(i) = by_area_pos.get(&area_uuid).copied() {
                        i
                    } else {
                        let i = grouped.by_area.len();
                        grouped
                            .by_area
                            .push((area_uuid.clone(), AreaGroup::default()));
                        by_area_pos.insert(area_uuid.clone(), i);
                        i
                    };
                    grouped.by_area[area_idx].1.tasks.push(task);
                } else {
                    grouped.unscoped.push(task);
                }
                continue;
            }

            let project_uuid = self.store.effective_project_uuid(task);
            let area_uuid = self.store.effective_area_uuid(task);

            match (project_uuid, area_uuid) {
                (Some(project_uuid), _) => {
                    // Tasks in a project are grouped under the project header
                    // even if that project belongs to an area.
                    let project_idx = if let Some(i) = project_only_pos.get(&project_uuid).copied()
                    {
                        i
                    } else {
                        let i = grouped.project_only.len();
                        grouped
                            .project_only
                            .push((project_uuid.clone(), Vec::new()));
                        project_only_pos.insert(project_uuid.clone(), i);
                        i
                    };
                    grouped.project_only[project_idx].1.push(task);
                }
                (None, Some(area_uuid)) => {
                    let area_idx = if let Some(i) = by_area_pos.get(&area_uuid).copied() {
                        i
                    } else {
                        let i = grouped.by_area.len();
                        grouped
                            .by_area
                            .push((area_uuid.clone(), AreaGroup::default()));
                        by_area_pos.insert(area_uuid.clone(), i);
                        i
                    };
                    grouped.by_area[area_idx].1.tasks.push(task);
                }
                (None, None) => {
                    grouped.unscoped.push(task);
                }
            }
        }

        grouped
    }

    fn regular_block_count(grouped: &GroupedSection<'_>) -> usize {
        let mut count = 0usize;
        if !grouped.unscoped.is_empty() {
            count += 1;
        }
        count += grouped.project_only.len();
        count += grouped.by_area.len();
        count
    }

    fn regular_grouped_height(&'a self, grouped: &GroupedSection<'a>) -> u16 {
        let mut h = 0u16;

        if !grouped.unscoped.is_empty() {
            h = h.saturating_add(
                TaskGroupWidget {
                    header: None,
                    tasks: self.tasks_widget(&grouped.unscoped, false, false),
                    indent_under_header: 0,
                }
                .height(),
            );
        }

        for (project_uuid, tasks) in &grouped.project_only {
            h = h.saturating_add(1);
            h = h.saturating_add(
                TaskGroupWidget {
                    header: Some(TaskGroupHeader::Project {
                        project_uuid: project_uuid.clone(),
                        title: self.store.resolve_project_title(project_uuid),
                        id_prefix_len: self.id_prefix_len,
                    }),
                    tasks: self.tasks_widget(tasks.as_slice(), false, false),
                    indent_under_header: 2,
                }
                .height(),
            );
        }

        for (area_uuid, area_group) in &grouped.by_area {
            h = h.saturating_add(1);
            h = h.saturating_add(
                TaskGroupWidget {
                    header: Some(TaskGroupHeader::Area {
                        area_uuid: area_uuid.clone(),
                        title: self.store.resolve_area_title(area_uuid),
                        id_prefix_len: self.id_prefix_len,
                    }),
                    tasks: self.tasks_widget(&area_group.tasks, false, false),
                    indent_under_header: 2,
                }
                .height(),
            );
        }

        let blocks = Self::regular_block_count(grouped);
        if blocks > 1 {
            h = h.saturating_add((blocks - 1) as u16);
        }
        h
    }

    fn evening_items(&'a self) -> Vec<&'a Task> {
        self.items.iter().filter(|task| task.evening).collect()
    }

    fn render_regular_grouped_section(
        &'a self,
        grouped: &GroupedSection<'a>,
        mut y: u16,
        base_x: u16,
        base_width: u16,
        buf: &mut Buffer,
    ) -> u16 {
        let mut first_block = true;

        if !grouped.unscoped.is_empty() {
            let group = TaskGroupWidget {
                header: None,
                tasks: self.tasks_widget(&grouped.unscoped, false, false),
                indent_under_header: 0,
            };
            let h = group.height();
            group.render(
                Rect {
                    x: base_x,
                    y,
                    width: base_width,
                    height: h,
                },
                buf,
            );
            y = y.saturating_add(h);
            first_block = false;
        }

        for (project_uuid, tasks) in &grouped.project_only {
            if !first_block {
                y = y.saturating_add(1);
            }
            let group = TaskGroupWidget {
                header: Some(TaskGroupHeader::Project {
                    project_uuid: project_uuid.clone(),
                    title: self.store.resolve_project_title(project_uuid),
                    id_prefix_len: self.id_prefix_len,
                }),
                tasks: self.tasks_widget(tasks.as_slice(), false, false),
                indent_under_header: 2,
            };
            let h = group.height();
            group.render(
                Rect {
                    x: base_x,
                    y,
                    width: base_width,
                    height: h,
                },
                buf,
            );
            y = y.saturating_add(h);
            first_block = false;
        }

        for (area_uuid, area_group) in &grouped.by_area {
            if !first_block {
                y = y.saturating_add(1);
            }
            let group = TaskGroupWidget {
                header: Some(TaskGroupHeader::Area {
                    area_uuid: area_uuid.clone(),
                    title: self.store.resolve_area_title(area_uuid),
                    id_prefix_len: self.id_prefix_len,
                }),
                tasks: self.tasks_widget(&area_group.tasks, false, false),
                indent_under_header: 2,
            };
            let h = group.height();
            group.render(
                Rect {
                    x: base_x,
                    y,
                    width: base_width,
                    height: h,
                },
                buf,
            );
            y = y.saturating_add(h);

            first_block = false;
        }

        y
    }

    pub fn height(&'a self) -> u16 {
        if self.is_empty() {
            return 1;
        }

        let mut h: u16 = 1;

        if self.has_regular() {
            h = h.saturating_add(1);
            let regular = self.group_regular_items();
            h = h.saturating_add(self.regular_grouped_height(&regular));
        }

        if self.has_evening() {
            h = h.saturating_add(1);
            h = h.saturating_add(1);
            h = h.saturating_add(1);
            let evening_items = self.evening_items();
            h = h.saturating_add(self.tasks_widget(&evening_items, true, true).height());
        }

        h
    }
}

impl<'a> Widget for TodayView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        if self.is_empty() {
            Line::from(Span::styled("No tasks for today.", Self::dim())).render(
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            return;
        }

        let mut y = area.y;

        Line::from(Span::styled(
            self.header_text(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ))
        .render(
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
            buf,
        );
        y = y.saturating_add(1);

        if self.has_regular() {
            y = y.saturating_add(1);
            let regular = self.group_regular_items();
            y = self.render_regular_grouped_section(
                &regular,
                y,
                area.x.saturating_add(2),
                area.width.saturating_sub(2),
                buf,
            );
        }

        if self.has_evening() {
            y = y.saturating_add(1);
            Line::from(Span::styled(
                format!("{} This Evening", ICONS.evening),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Blue),
            ))
            .render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            y = y.saturating_add(1);
            y = y.saturating_add(1);

            let evening_items = self.evening_items();
            let evening_tasks = self.tasks_widget(&evening_items, true, true);
            let evening_h = evening_tasks.height();
            evening_tasks.render(
                Rect {
                    x: area.x.saturating_add(2),
                    y,
                    width: area.width.saturating_sub(2),
                    height: evening_h,
                },
                buf,
            );
        }
    }
}
