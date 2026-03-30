use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;
use std::sync::Arc;

const LIST_INDENT: u32 = 2;

fn id_prefix_len(store: &ThingsStore, items: &[Task]) -> usize {
    let ids = items
        .iter()
        .map(|task| task.uuid.clone())
        .collect::<Vec<_>>();
    store.unique_prefix_length(&ids)
}

#[derive(Default, Props)]
pub struct InboxViewProps<'a> {
    pub items: Option<&'a Vec<Task>>,
    pub detailed: bool,
}

#[component]
pub fn InboxView<'a>(hooks: Hooks, props: &InboxViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element!(Text(content: "")).into_any();
    };

    let content: AnyElement<'a> = {
        if items.is_empty() {
            element! { Text(content: "Inbox is empty.", wrap: TextWrap::NoWrap) }.into_any()
        } else {
            let prefix_len = id_prefix_len(store.as_ref(), items);
            let refs = items.iter().collect::<Vec<_>>();
            element! {
                    View(flex_direction: FlexDirection::Column) {
                        Text(content: format!("{} Inbox  ({} tasks)", ICONS.inbox, items.len()), wrap: TextWrap::NoWrap)
                        Text(content: "", wrap: TextWrap::NoWrap)
                        View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                            TaskList(
                                items: refs,
                                id_prefix_len: prefix_len,
                                options: TaskOptions {
                                    detailed: props.detailed,
                                    show_project: false,
                                    show_area: false,
                                    show_today_markers: true,
                                    show_staged_today_marker: false,
                                },
                            )
                        }
                    }
                }
                .into_any()
        }
    };

    content
}
