use crate::common::ICONS;
use crate::store::Area;
use crate::ui::components::empty_text::EmptyText;
use crate::ui::components::id::Id;
use crate::ui::components::tags_badge::TagsBadge;
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct AreasViewProps {
    pub areas: Vec<Area>,
    pub id_prefix_len: usize,
}

#[component]
pub fn AreasView<'a>(props: &'a AreasViewProps) -> impl Into<AnyElement<'a>> {
    if props.areas.is_empty() {
        return element!(EmptyText(content: "No areas.")).into_any();
    }

    element! {
        View(flex_direction: FlexDirection::Column, gap: 1) {
            Text(
                content: format!("{} Areas  ({})", ICONS.area, props.areas.len()),
                color: Color::Magenta,
                weight: Weight::Bold,
                wrap: TextWrap::NoWrap,
            )
            #(props.areas.iter().map(|area| element! {
                View(flex_direction: FlexDirection::Row, gap: 2, padding_left: 2) {
                    View(flex_direction: FlexDirection::Row, gap: 1) {
                        Id(id: &area.uuid, length: props.id_prefix_len)
                        Text(content: ICONS.area, color: Color::DarkGrey)
                        Text(content: area.title.clone(), wrap: TextWrap::NoWrap)
                    }
                    TagsBadge(tags: area.tags.clone())
                }
            }))
        }
    }
    .into_any()
}
