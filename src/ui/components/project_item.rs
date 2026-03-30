use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use crate::ui::components::details_container::DetailsContainer;
use crate::ui::components::task_line::TaskLine;
use crate::ui::components::tasks::TaskOptions;
use crate::ui::utils::id_prefix;
use iocraft::prelude::*;
use std::sync::Arc;

#[derive(Default, Props)]
pub struct ProjectItemProps<'a> {
    pub project: Option<&'a Task>,
    pub options: TaskOptions,
    pub id_prefix_len: usize,
}

#[component]
pub fn ProjectItem<'a>(props: &ProjectItemProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(project) = props.project else {
        return element!(Fragment).into_any();
    };

    let details = if props.options.detailed {
        element!(ProjectDetails(project: project)).into_any()
    } else {
        element!(Fragment).into_any()
    };

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: id_prefix(&project.uuid, props.id_prefix_len))
            View(flex_direction: FlexDirection::Column) {
                ProjectText(project: project, options: props.options)
                #(details)
            }
        }
    }
    .into_any()
}

#[derive(Default, Props)]
pub struct ProjectTextProps<'a> {
    pub project: Option<&'a Task>,
    pub options: TaskOptions,
}

#[component]
pub fn ProjectText<'a>(hooks: Hooks, props: &ProjectTextProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(project) = props.project else {
        return element!(Fragment).into_any();
    };

    let store = hooks.use_context::<Arc<ThingsStore>>().clone();

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: progress_marker(project, store.as_ref()))
            TaskLine(
                task: project,
                show_today_markers: props.options.show_today_markers,
                show_staged_today_marker: props.options.show_staged_today_marker,
                show_tags: true,
                show_project: false,
                show_area: props.options.show_area,
            )
        }
    }
    .into_any()
}

#[derive(Default, Props)]
pub struct ProjectDetailsProps<'a> {
    pub project: Option<&'a Task>,
}

#[component]
pub fn ProjectDetails<'a>(props: &ProjectDetailsProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(project) = props.project else {
        return element!(Fragment).into_any();
    };

    let note_text = project.notes.as_deref().unwrap_or("").trim();
    if note_text.is_empty() {
        return element!(Fragment).into_any();
    }

    element! {
        DetailsContainer {
            Text(content: note_text, wrap: TextWrap::NoWrap)
        }
    }
    .into_any()
}

fn progress_marker(project: &Task, store: &ThingsStore) -> &'static str {
    if project.in_someday() {
        return ICONS.anytime;
    }

    let progress = store.project_progress(&project.uuid);
    let total = progress.total;
    let done = progress.done;

    if total == 0 || done == 0 {
        return ICONS.progress_empty;
    }

    if done == total {
        return ICONS.progress_full;
    }

    let ratio = done as f32 / total as f32;
    if ratio < (1.0 / 3.0) {
        return ICONS.progress_quarter;
    }

    if ratio < (2.0 / 3.0) {
        return ICONS.progress_half;
    }

    ICONS.progress_three_quarter
}
