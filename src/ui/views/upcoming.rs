use crate::common::{fmt_date, ICONS};
use crate::store::{Task, ThingsStore};
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;
use std::sync::Arc;

const LIST_INDENT: u32 = 4;

#[derive(Default, Props)]
pub struct UpcomingViewProps<'a> {
    pub items: Option<&'a Vec<Task>>,
    pub detailed: bool,
}

#[component]
pub fn UpcomingView<'a>(hooks: Hooks, props: &UpcomingViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element! { Text(content: "") }.into_any();
    };

    if items.is_empty() {
        return element! { Text(content: "No upcoming tasks.", wrap: TextWrap::NoWrap) }.into_any();
    }

    let id_prefix_len = store.unique_prefix_length(&items.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());

    let mut groups: Vec<(String, Vec<&Task>)> = Vec::new();
    for task in items {
        let day = fmt_date(task.start_date);
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
        show_project: false,
        show_area: false,
        show_today_markers: true,
        show_staged_today_marker: false,
    };

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
            Text(content: format!("{} Upcoming  ({} tasks)", ICONS.upcoming, items.len()), wrap: TextWrap::NoWrap)
            Text(content: "", wrap: TextWrap::NoWrap)
            #(sections)
        }
    }
    .into_any()
}
