use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use crate::ui::components::empty_text::EmptyText;
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;
use std::sync::Arc;

const LIST_INDENT: u32 = 2;

#[derive(Default, Props)]
pub struct SomedayViewProps<'a> {
    pub items: Option<&'a Vec<Task>>,
    pub detailed: bool,
}

#[component]
pub fn SomedayView<'a>(hooks: Hooks, props: &SomedayViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element! { Text(content: "") }.into_any();
    };

    if items.is_empty() {
        return element! { EmptyText(content: "Someday is empty.") }.into_any();
    }

    let id_prefix_len =
        store.unique_prefix_length(&items.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());
    let projects = items.iter().filter(|i| i.is_project()).collect::<Vec<_>>();
    let tasks = items.iter().filter(|i| !i.is_project()).collect::<Vec<_>>();
    let has_projects = !projects.is_empty();
    let has_tasks = !tasks.is_empty();

    let options = TaskOptions {
        detailed: props.detailed,
        show_project: false,
        show_area: false,
        show_today_markers: false,
        show_staged_today_marker: false,
    };

    element! {
        View(flex_direction: FlexDirection::Column, gap: 1) {
            Text(
                content: format!("{} Someday  ({} items)", ICONS.task_someday, items.len()),
                wrap: TextWrap::NoWrap,
                color: Color::Cyan,
                weight: Weight::Bold,
            )

            #(if has_projects {
                Some(element! {
                    View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                        TaskList(items: projects.clone(), id_prefix_len, options)
                    }
                })
            } else { None })

            #(if has_tasks {
                Some(element! {
                    View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                        TaskList(items: tasks.clone(), id_prefix_len, options)
                    }
                })
            } else { None })
        }
    }
    .into_any()
}
