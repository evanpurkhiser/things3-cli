use crate::ids::matching::longest_shortest_unique_prefix_len;
use crate::store::ChecklistItem;
use crate::ui::components::checklist_item::CheckListRow;
use crate::ui::utils::id_prefix;
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct CheckListProps<'a> {
    pub items: Option<&'a [ChecklistItem]>,
    pub show_ids: bool,
    pub shift_left: bool,
}

#[component]
pub fn CheckList<'a>(props: &CheckListProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(items) = props.items else {
        return element!(Fragment).into_any();
    };

    let prefix_len = prefix_len(items, props.show_ids);
    let id_prefixes = items
        .iter()
        .map(|item| id_prefix(&item.uuid, prefix_len))
        .collect::<Vec<_>>();

    let margin_left = if props.shift_left && prefix_len > 0 {
        Margin::Length(-(prefix_len as i32 + 3))
    } else {
        Margin::Length(0)
    };

    let items = items.iter().enumerate().map(move |(i, item)| {
        let is_last = i == items.len() - 1;

        let id = if prefix_len > 0 {
            Some(id_prefixes[i].clone())
        } else {
            None
        };
        element!(CheckListRow(item, id, is_last))
    });

    element! {
        View(flex_direction: FlexDirection::Column, margin_left) {
            #(items)
        }
    }
    .into_any()
}

fn prefix_len(items: &[ChecklistItem], show_ids: bool) -> usize {
    if !show_ids || items.is_empty() {
        return 0;
    }

    let ids = items.iter().map(|i| i.uuid.clone()).collect::<Vec<_>>();
    longest_shortest_unique_prefix_len(&ids)
}
