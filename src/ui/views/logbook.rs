use crate::common::{ICONS, fmt_date_local};
use crate::store::{Task, ThingsStore};
use crate::ui::components::empty_text::EmptyText;
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
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element! { Text(content: "") }.into_any();
    };

    if items.is_empty() {
        return element! { EmptyText(content: "Logbook is empty.") }.into_any();
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

    let id_prefix_len =
        store.unique_prefix_length(&items.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());
    let mut sections: Vec<AnyElement<'a>> = Vec::new();
    for (day, day_items) in groups.into_iter() {
        sections.push(
            element! {
                View(flex_direction: FlexDirection::Column) {
                    Text(content: format!("  {}", day), wrap: TextWrap::NoWrap, weight: Weight::Bold)
                    View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                        TaskList(items: day_items, id_prefix_len: id_prefix_len, options)
                    }
                }
            }
            .into_any(),
        );
    }

    element! {
        View(flex_direction: FlexDirection::Column, gap: 1) {
            Text(
                content: format!("{} Logbook  ({} tasks)", ICONS.done, items.len()),
                wrap: TextWrap::NoWrap,
                color: Color::Green,
                weight: Weight::Bold,
            )
            View(flex_direction: FlexDirection::Column, gap: 1) {
                #(sections)
            }
        }
    }
    .into_any()
}
