use crate::common::{fmt_deadline, ICONS};
use crate::store::{Task, ThingsStore};
use crate::ui::components::details_container::DetailsContainer;
use crate::ui::components::tasks::{TaskList, TaskOptions};
use chrono::{DateTime, Utc};
use iocraft::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProjectHeadingGroup<'a> {
    pub title: String,
    pub items: Vec<&'a Task>,
}

#[derive(Default, Props)]
pub struct ProjectViewProps<'a> {
    pub project: Option<&'a Task>,
    pub ungrouped: Vec<&'a Task>,
    pub heading_groups: Vec<ProjectHeadingGroup<'a>>,
    pub detailed: bool,
    pub no_color: bool,
}

#[component]
pub fn ProjectView<'a>(hooks: Hooks, props: &ProjectViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let today = *hooks.use_context::<DateTime<Utc>>();
    let Some(project) = props.project else {
        return element! { Text(content: "") }.into_any();
    };

    let progress = store.project_progress(&project.uuid);
    let total = progress.total;
    let done = progress.done;

    let tags = if project.tags.is_empty() {
        String::new()
    } else {
        let tag_names = project
            .tags
            .iter()
            .map(|t| store.resolve_tag_title(t))
            .collect::<Vec<_>>()
            .join(", ");
        format!(" [{}]", tag_names)
    };
    let deadline = fmt_deadline(project.deadline, &today, props.no_color);

    let mut all_uuids = props
        .ungrouped
        .iter()
        .map(|t| t.uuid.clone())
        .collect::<Vec<_>>();
    for group in &props.heading_groups {
        all_uuids.extend(group.items.iter().map(|t| t.uuid.clone()));
    }
    let id_prefix_len = store.unique_prefix_length(&all_uuids);

    let options = TaskOptions {
        detailed: props.detailed,
        show_project: false,
        show_area: false,
        show_today_markers: true,
        show_staged_today_marker: false,
    };

    let note_lines = project
        .notes
        .as_deref()
        .unwrap_or("")
        .lines()
        .map(|line| element! { Text(content: line, wrap: TextWrap::NoWrap) }.into_any())
        .collect::<Vec<_>>();

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(
                content: format!(
                    "{} {}  ({}/{}){}{}",
                    ICONS.project,
                    project.title,
                    done,
                    done + total,
                    deadline,
                    tags
                ),
                wrap: TextWrap::NoWrap,
            )

            #(if !note_lines.is_empty() {
                Some(element! {
                    View(flex_direction: FlexDirection::Column, padding_left: 2) {
                        DetailsContainer {
                            #(note_lines)
                        }
                    }
                })
            } else { None })

            #(if props.ungrouped.is_empty() && props.heading_groups.is_empty() {
                Some(element! {
                    Text(content: "  No tasks.", wrap: TextWrap::NoWrap)
                })
            } else { None })

            #(if !props.ungrouped.is_empty() {
                Some(element! {
                    View(flex_direction: FlexDirection::Column) {
                        Text(content: "", wrap: TextWrap::NoWrap)
                        View(flex_direction: FlexDirection::Column, padding_left: 2) {
                            TaskList(items: props.ungrouped.clone(), id_prefix_len, options)
                        }
                    }
                })
            } else { None })

            #(props.heading_groups.iter().map(|group| element! {
                View(flex_direction: FlexDirection::Column) {
                    Text(content: "", wrap: TextWrap::NoWrap)
                    Text(content: format!("  {}", group.title), wrap: TextWrap::NoWrap)
                    View(flex_direction: FlexDirection::Column, padding_left: 4) {
                        TaskList(items: group.items.clone(), id_prefix_len, options)
                    }
                }
            }))
        }
    }
    .into_any()
}
