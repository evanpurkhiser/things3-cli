use crate::common::ICONS;
use crate::ids::ThingsId;
use crate::store::Task;
use crate::ui::components::empty_text::EmptyText;
use crate::ui::components::id::Id;
use crate::ui::components::tasks::{TaskList, TaskOptions};
use iocraft::prelude::*;

#[derive(Clone)]
pub struct ProjectsAreaGroup {
    pub area_uuid: ThingsId,
    pub area_title: String,
    pub projects: Vec<Task>,
}

#[derive(Default, Props)]
pub struct ProjectsViewProps {
    pub projects_count: usize,
    pub no_area_projects: Vec<Task>,
    pub area_groups: Vec<ProjectsAreaGroup>,
    pub detailed: bool,
    pub id_prefix_len: usize,
}

#[component]
pub fn ProjectsView<'a>(props: &'a ProjectsViewProps) -> impl Into<AnyElement<'a>> {
    if props.projects_count == 0 {
        return element! { EmptyText(content: "No active projects.") }.into_any();
    }

    let options = TaskOptions {
        detailed: props.detailed,
        show_project: false,
        show_area: false,
        show_today_markers: true,
        show_staged_today_marker: false,
    };

    let free_projects = if !props.no_area_projects.is_empty() {
        element! {
            View(flex_direction: FlexDirection::Column, padding_left: 2) {
                TaskList(
                    items: props.no_area_projects.iter().collect::<Vec<_>>(),
                    id_prefix_len: props.id_prefix_len,
                    options,
                )
            }
        }
        .into_any()
    } else {
        element!(Fragment).into_any()
    };

    let project_areas = props.area_groups.iter().map(|group| {
        element! {
            ProjectsAreaSection(group, id_prefix_len: props.id_prefix_len, options)
        }
    });

    element! {
        View(flex_direction: FlexDirection::Column) {
            Text(
                content: format!("● Projects  ({})", props.projects_count),
                color: Color::Green,
                weight: Weight::Bold,
                wrap: TextWrap::NoWrap,
            )
            Text(content: "", wrap: TextWrap::NoWrap)
            #(free_projects)
            #(project_areas)
        }
    }
    .into_any()
}

#[derive(Default, Props)]
struct ProjectsAreaSectionProps<'a> {
    pub group: Option<&'a ProjectsAreaGroup>,
    pub id_prefix_len: usize,
    pub options: TaskOptions,
}

#[component]
fn ProjectsAreaSection<'a>(props: &ProjectsAreaSectionProps<'a>) -> impl Into<AnyElement<'a>> {
    let Some(group) = props.group else {
        return element!(Fragment).into_any();
    };

    element! {
        View(flex_direction: FlexDirection::Column, padding_left: 2) {
            Text(content: "", wrap: TextWrap::NoWrap)
            View(flex_direction: FlexDirection::Row, gap: 1) {
                Id(id: &group.area_uuid, length: props.id_prefix_len)
                Text(content: ICONS.area, color: Color::DarkGrey)
                Text(content: &group.area_title, wrap: TextWrap::NoWrap, weight: Weight::Bold)
            }
            View(flex_direction: FlexDirection::Column, padding_left: 2) {
                TaskList(
                    items: group.projects.iter().collect::<Vec<_>>(),
                    id_prefix_len: props.id_prefix_len,
                    options: props.options,
                )
            }
        }
    }
    .into_any()
}
