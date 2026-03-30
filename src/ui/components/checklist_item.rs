use crate::common::ICONS;
use crate::store::ChecklistItem;
use iocraft::prelude::*;

/// A single checklist-item row.
///
/// When `id` is `Some`, it is rendered in a fixed-width left column so
/// connectors follow the ID prefix.
///
/// ```text
/// M ├╴○ Confirm changelog
/// J └╴● Tag release commit   (is_last)
/// ```
///
/// When `id` is `None` (no IDs), the connector starts at column 0:
/// ```text
/// ├╴○ title
/// └╴● title
/// ```

#[derive(Default, Props)]
pub struct CheckListRowProps<'a> {
    pub item: Option<&'a ChecklistItem>,
    pub id: Option<String>,
    pub is_last: bool,
}

#[component]
pub fn CheckListRow<'a>(props: &CheckListRowProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(item) = props.item else {
        return element!(Fragment).into_any();
    };

    let connector = if props.is_last { "└╴" } else { "├╴" };

    let id_prefix = props
        .id
        .as_ref()
        .map(|id| element!(Text(content: id.clone())).into_any());

    element!(View {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            #(id_prefix)
            Text(content: connector)
        }
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: checklist_icon(item))
            Text(content: item.title.clone())
        }
    })
    .into_any()
}

fn checklist_icon(item: &ChecklistItem) -> &'static str {
    if item.is_completed() {
        ICONS.checklist_done
    } else if item.is_canceled() {
        ICONS.checklist_canceled
    } else {
        ICONS.checklist_open
    }
}
