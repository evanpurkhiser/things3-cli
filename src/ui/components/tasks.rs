use crate::store::Task;
use crate::ui::components::project_item::ProjectItem;
use crate::ui::components::task_item::TaskItem;
use crate::wire::task::TaskType;
use iocraft::prelude::*;

#[derive(Clone, Copy, Default)]
pub struct TaskOptions {
    pub detailed: bool,
    pub show_project: bool,
    pub show_area: bool,
    pub show_today_markers: bool,
    pub show_staged_today_marker: bool,
}

#[derive(Default, Props)]
pub struct TaskListProps<'a> {
    pub items: Vec<&'a Task>,
    pub id_prefix_len: usize,
    pub options: TaskOptions,
}

#[component]
pub fn TaskList<'a>(props: &TaskListProps<'a>) -> impl Into<AnyElement<'a>> {
    let items = props.items.iter().map(|item| match item.item_type {
        TaskType::Todo => element! {
            TaskItem(
                task: *item,
                options: props.options,
                id_prefix_len: props.id_prefix_len,
            )
        }
        .into_any(),
        TaskType::Project => element! {
            ProjectItem(
                project: *item,
                options: props.options,
                id_prefix_len: props.id_prefix_len,
            )
        }
        .into_any(),
        _ => element!(Fragment).into_any(),
    });

    element! {
        View(flex_direction: FlexDirection::Column) { #(items) }
    }
    .into_any()
}
