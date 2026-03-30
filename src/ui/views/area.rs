use crate::common::ICONS;
use crate::store::{Area, Task, ThingsStore};
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;
use std::sync::Arc;

const LIST_INDENT: u32 = 2;

#[derive(Default, Props)]
pub struct AreaViewProps<'a> {
    pub area: Option<&'a Area>,
    pub tasks: Vec<&'a Task>,
    pub projects: Vec<&'a Task>,
    pub detailed: bool,
}

#[component]
pub fn AreaView<'a>(hooks: Hooks, props: &AreaViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(area) = props.area else {
        return element! { Text(content: "") }.into_any();
    };

    let project_count = props.projects.len();
    let task_count = props.tasks.len();

    let mut parts = Vec::new();
    if project_count > 0 {
        parts.push(format!(
            "{} project{}",
            project_count,
            if project_count == 1 { "" } else { "s" }
        ));
    }
    if task_count > 0 {
        parts.push(format!(
            "{} task{}",
            task_count,
            if task_count == 1 { "" } else { "s" }
        ));
    }
    let count_str = if parts.is_empty() {
        String::new()
    } else {
        format!("  ({})", parts.join(", "))
    };

    let tags = if area.tags.is_empty() {
        String::new()
    } else {
        let names = area
            .tags
            .iter()
            .map(|t| store.resolve_tag_title(t))
            .collect::<Vec<_>>()
            .join(", ");
        format!(" [{}]", names)
    };

    let mut all_uuids = vec![area.uuid.clone()];
    all_uuids.extend(props.projects.iter().map(|p| p.uuid.clone()));
    all_uuids.extend(props.tasks.iter().map(|t| t.uuid.clone()));
    let id_prefix_len = store.unique_prefix_length(&all_uuids);

    let task_options = TaskOptions {
        detailed: props.detailed,
        show_project: false,
        show_area: false,
        show_today_markers: true,
        show_staged_today_marker: false,
    };
    let project_options = TaskOptions {
        detailed: props.detailed,
        show_project: false,
        show_area: false,
        show_today_markers: false,
        show_staged_today_marker: false,
    };

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(content: format!("{} {}{}{}", ICONS.area, area.title, count_str, tags), wrap: TextWrap::NoWrap)

            #(if !props.tasks.is_empty() {
                Some(element! {
                    View(flex_direction: FlexDirection::Column) {
                        Text(content: "", wrap: TextWrap::NoWrap)
                        View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                            TaskList(items: props.tasks.clone(), id_prefix_len, options: task_options)
                        }
                    }
                })
            } else { None })

            #(if !props.projects.is_empty() {
                Some(element! {
                    View(flex_direction: FlexDirection::Column) {
                        Text(content: "", wrap: TextWrap::NoWrap)
                        View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                            TaskList(items: props.projects.clone(), id_prefix_len, options: project_options)
                        }
                    }
                })
            } else { None })
        }
    }
    .into_any()
}
