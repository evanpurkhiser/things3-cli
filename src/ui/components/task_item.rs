use crate::common::ICONS;
use crate::store::Task;
use crate::ui::components::checklist::CheckList;
use crate::ui::components::details_container::DetailsContainer;
use crate::ui::components::task_line::TaskLine;
use crate::ui::components::tasks::TaskOptions;
use crate::ui::utils::id_prefix;
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct TaskItemProps<'a> {
    pub task: Option<&'a Task>,
    pub options: TaskOptions,
    pub id_prefix_len: usize,
}

#[component]
pub fn TaskItem<'a>(props: &TaskItemProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(task) = props.task else {
        return element!(Fragment).into_any();
    };

    let details = if props.options.detailed {
        element! {
            TaskDetails(task: task, id_prefix_len: props.id_prefix_len)
        }
        .into_any()
    } else {
        element!(Fragment).into_any()
    };

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: id_prefix(&task.uuid, props.id_prefix_len))
            View(flex_direction: FlexDirection::Column) {
                TaskText(task: task, options: props.options)
                #(details)
            }
        }
    }
    .into_any()
}

#[derive(Default, Props)]
pub struct TaskTextProps<'a> {
    pub task: Option<&'a Task>,
    pub options: TaskOptions,
}

#[component]
pub fn TaskText<'a>(props: &TaskTextProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(task) = props.task else {
        return element!(Fragment).into_any();
    };

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: checkbox_str(task))
            TaskLine(
                task: task,
                show_today_markers: props.options.show_today_markers,
                show_staged_today_marker: props.options.show_staged_today_marker,
                show_tags: true,
                show_project: props.options.show_project,
                show_area: props.options.show_area,
            )
        }
    }
    .into_any()
}

#[derive(Default, Props)]
pub struct TaskDetailProps<'a> {
    pub task: Option<&'a Task>,
    pub id_prefix_len: usize,
}

#[component]
pub fn TaskDetails<'a>(props: &TaskDetailProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(task) = props.task else {
        return element!(Fragment).into_any();
    };

    let show_ids = props.id_prefix_len > 0;
    let note_text = task.notes.as_deref().unwrap_or("").trim();

    let checklist_items = if task.checklist_items.is_empty() {
        None
    } else {
        Some(task.checklist_items.as_slice())
    };

    if note_text.is_empty() && checklist_items.is_none() {
        return element!(Fragment).into_any();
    }

    let note_text = if note_text.is_empty() {
        element!(Fragment).into_any()
    } else {
        element!(Text(content: note_text)).into_any()
    };

    element! {
        DetailsContainer {
            View(flex_direction: FlexDirection::Column, gap: 1) {
                #(note_text)
                CheckList(items: checklist_items, show_ids, shift_left: true)
            }
        }

    }
    .into_any()
}

fn checkbox_str(task: &Task) -> &'static str {
    if task.is_completed() {
        ICONS.task_done
    } else if task.is_canceled() {
        ICONS.task_canceled
    } else if task.in_someday() {
        ICONS.task_someday
    } else {
        ICONS.task_open
    }
}
