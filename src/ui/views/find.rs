use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use crate::ui::components::project_item::ProjectItem;
use crate::ui::components::task_item::TaskItem;
use crate::ui::components::tasks::TaskOptions;
use iocraft::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct FindRow<'a> {
    pub task: &'a Task,
    pub force_detailed: bool,
}

#[derive(Default, Props)]
pub struct FindViewProps<'a> {
    pub rows: Vec<FindRow<'a>>,
    pub detailed: bool,
}

#[component]
pub fn FindView<'a>(hooks: Hooks, props: &FindViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();

    if props.rows.is_empty() {
        return element! { Text(content: "No matching tasks.", wrap: TextWrap::NoWrap) }.into_any();
    }

    let id_prefix_len = store.unique_prefix_length(
        &props
            .rows
            .iter()
            .map(|row| row.task.uuid.clone())
            .collect::<Vec<_>>(),
    );

    let count = props.rows.len();
    let label = if count == 1 { "task" } else { "tasks" };

    let mut body: Vec<AnyElement<'a>> = Vec::new();
    for row in &props.rows {
        let options = TaskOptions {
            detailed: props.detailed || row.force_detailed,
            show_project: true,
            show_area: false,
            show_today_markers: true,
            show_staged_today_marker: false,
        };
        let line = if row.task.is_project() {
            element! {
                View(flex_direction: FlexDirection::Column, padding_left: 2) {
                    ProjectItem(project: row.task, options, id_prefix_len)
                }
            }
            .into_any()
        } else {
            element! {
                View(flex_direction: FlexDirection::Column, padding_left: 2) {
                    TaskItem(task: row.task, options, id_prefix_len)
                }
            }
            .into_any()
        };
        body.push(line);
    }

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(content: format!("{} Find  ({} {})", ICONS.tag, count, label), wrap: TextWrap::NoWrap)
            Text(content: "", wrap: TextWrap::NoWrap)
            #(body)
        }
    }
    .into_any()
}
