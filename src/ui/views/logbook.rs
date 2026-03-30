use crate::common::{fmt_date_local, ICONS};
use crate::store::{Task, ThingsStore};
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;
use std::sync::Arc;

const LIST_INDENT: u32 = 4;

#[derive(Default, Props)]
pub struct LogbookViewProps<'a> {
    pub items: Option<&'a Vec<Task>>,
    pub detailed: bool,
}

#[component]
pub fn LogbookView<'a>(hooks: Hooks, props: &LogbookViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let _store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element! { Text(content: "") }.into_any();
    };

    if items.is_empty() {
        return element! { Text(content: "Logbook is empty.", wrap: TextWrap::NoWrap) }.into_any();
    }

    let mut groups: Vec<(String, Vec<&Task>)> = Vec::new();
    for task in items {
        let day = fmt_date_local(task.stop_date);
        if let Some((current_day, day_tasks)) = groups.last_mut()
            && *current_day == day
        {
            day_tasks.push(task);
        } else {
            groups.push((day, vec![task]));
        }
    }

    let options = TaskOptions {
        detailed: props.detailed,
        show_project: true,
        show_area: false,
        show_today_markers: false,
        show_staged_today_marker: false,
    };

    let id_prefix_len = 0usize;
    let mut sections: Vec<AnyElement<'a>> = Vec::new();
    for (idx, (day, day_items)) in groups.into_iter().enumerate() {
        if idx > 0 {
            sections.push(element! { Text(content: "", wrap: TextWrap::NoWrap) }.into_any());
        }
        sections.push(
            element! {
                View(flex_direction: FlexDirection::Column) {
                    Text(content: format!("  {}", day), wrap: TextWrap::NoWrap)
                    View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                        TaskList(items: day_items, id_prefix_len, options)
                    }
                }
            }
            .into_any(),
        );
    }

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(content: format!("{} Logbook  ({} tasks)", ICONS.done, items.len()), wrap: TextWrap::NoWrap)
            Text(content: "", wrap: TextWrap::NoWrap)
            #(sections)
        }
    }
    .into_any()
}
