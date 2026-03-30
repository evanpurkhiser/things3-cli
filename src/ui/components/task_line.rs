use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use chrono::{DateTime, Utc};
use iocraft::prelude::*;
use std::sync::Arc;

#[derive(Default, Props)]
pub struct TaskLineProps<'a> {
    pub task: Option<&'a Task>,
    pub show_today_markers: bool,
    pub show_staged_today_marker: bool,
    pub show_tags: bool,
    pub show_project: bool,
    pub show_area: bool,
}

#[component]
pub fn TaskLine<'a>(hooks: Hooks, props: &TaskLineProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(task) = props.task else {
        return element!(Fragment).into_any();
    };

    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let today = *hooks.use_context::<DateTime<Utc>>();

    let segments = vec![
        marker_element(
            task,
            &today,
            props.show_today_markers,
            props.show_staged_today_marker,
        ),
        title_element(task),
        tags_element(task, store.as_ref(), props.show_tags),
        context_element(task, store.as_ref(), props.show_project, props.show_area),
        deadline_element(task),
    ];

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            #(segments)
        }
    }
    .into_any()
}

fn marker_element<'a>(
    task: &Task,
    today: &DateTime<Utc>,
    show_today_markers: bool,
    show_staged_today_marker: bool,
) -> AnyElement<'a> {
    if show_today_markers {
        if task.evening {
            return element! { Text(content: ICONS.evening) }.into_any();
        }
        if task.is_today(today) {
            return element! { Text(content: ICONS.today) }.into_any();
        }
    }

    if show_staged_today_marker && task.is_staged_for_today(today) {
        return element! { Text(content: ICONS.today_staged) }.into_any();
    }

    element!(Fragment).into_any()
}

fn title_element<'a>(task: &Task) -> AnyElement<'a> {
    let content = if task.title.is_empty() {
        "(untitled)".to_string()
    } else {
        task.title.clone()
    };
    element! { Text(content: content) }.into_any()
}

fn tags_element<'a>(task: &Task, store: &ThingsStore, show_tags: bool) -> AnyElement<'a> {
    if !show_tags || task.tags.is_empty() {
        return element!(Fragment).into_any();
    }

    let tag_names: Vec<String> = task
        .tags
        .iter()
        .map(|t| store.resolve_tag_title(t))
        .collect();
    element! {
        Text(content: format!("[{}]", tag_names.join(", ")))
    }
    .into_any()
}

fn context_element<'a>(
    task: &Task,
    store: &ThingsStore,
    show_project: bool,
    show_area: bool,
) -> AnyElement<'a> {
    if show_project && let Some(proj) = store.effective_project_uuid(task) {
        let title = store.resolve_project_title(&proj);
        return element! {
            Text(content: format!("{} {}", ICONS.separator, title))
        }
        .into_any();
    }

    if show_area && let Some(area) = store.effective_area_uuid(task) {
        let title = store.resolve_area_title(&area);
        return element! {
            Text(content: format!("{} {}", ICONS.separator, title))
        }
        .into_any();
    }

    element!(Fragment).into_any()
}

fn deadline_element<'a>(task: &Task) -> AnyElement<'a> {
    let Some(deadline) = task.deadline else {
        return element!(Fragment).into_any();
    };

    let date_str = deadline.format("%Y-%m-%d").to_string();
    element! {
        Text(content: format!("{} due by {}", ICONS.deadline, date_str))
    }
    .into_any()
}
