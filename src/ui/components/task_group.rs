use crate::common::ICONS;
use crate::ids::ThingsId;
use crate::store::Task;
use crate::ui::components::header::Header;
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;

#[derive(Clone)]
pub enum TaskGroupHeader {
    Area {
        area_uuid: ThingsId,
        title: String,
        id_prefix_len: usize,
    },
    Project {
        project_uuid: ThingsId,
        title: String,
        id_prefix_len: usize,
    },
}

#[derive(Default, Props)]
pub struct TaskGroupProps<'a> {
    pub header: Option<TaskGroupHeader>,
    pub items: Vec<&'a Task>,
    pub id_prefix_len: usize,
    pub options: TaskOptions,
    pub indent_under_header: u16,
    pub hidden_count: usize,
}

#[component]
pub fn TaskGroup<'a>(props: &'a TaskGroupProps<'a>) -> impl Into<AnyElement<'a>> {
    let header_el = match &props.header {
        Some(TaskGroupHeader::Area {
            area_uuid,
            title,
            id_prefix_len,
        }) => element! {
            Header(
                uuid: area_uuid,
                title: title.as_str(),
                id_prefix_len: *id_prefix_len,
                icon: ICONS.area,
            )
        }
        .into_any(),
        Some(TaskGroupHeader::Project {
            project_uuid,
            title,
            id_prefix_len,
        }) => element! {
            Header(
                uuid: project_uuid,
                title: title.as_str(),
                id_prefix_len: *id_prefix_len,
                icon: ICONS.project,
            )
        }
        .into_any(),
        _ => element!(Fragment).into_any(),
    };

    let footer = if props.hidden_count > 0 {
        let text = format!("Hiding {} more", props.hidden_count);
        element!(Text(content: text, wrap: TextWrap::NoWrap)).into_any()
    } else {
        element!(Fragment).into_any()
    };

    element! {
        View(flex_direction: FlexDirection::Column) {
            #(header_el)
            View(
                flex_direction: FlexDirection::Column,
                padding_left: if props.header.is_some() { props.indent_under_header as u32 } else { 0 },
            ) {
                TaskList(
                    items: props.items.clone(),
                    id_prefix_len: props.id_prefix_len,
                    options: props.options,
                )
                #(footer)
            }
        }
    }
    .into_any()
}
