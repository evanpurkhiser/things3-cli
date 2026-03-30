use crate::common::ICONS;
use crate::ids::ThingsId;
use crate::store::{Task, ThingsStore};
use crate::ui::components::task_group::{TaskGroup, TaskGroupHeader};
use crate::ui::components::tasks::TaskOptions;
use iocraft::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

const MAX_GROUP_ITEMS: usize = 3;
const LIST_INDENT: u32 = 2;

#[derive(Default)]
struct AreaTaskGroup {
    tasks: Vec<usize>,
    projects: Vec<(ThingsId, Vec<usize>)>,
    project_pos: HashMap<ThingsId, usize>,
}

#[derive(Default)]
struct Grouped {
    unscoped: Vec<usize>,
    project_only: Vec<(ThingsId, Vec<usize>)>,
    by_area: Vec<(ThingsId, AreaTaskGroup)>,
}

fn ensure_area_group(
    grouped: &mut Grouped,
    area_pos: &mut HashMap<ThingsId, usize>,
    area_uuid: &ThingsId,
) -> usize {
    if let Some(i) = area_pos.get(area_uuid).copied() {
        return i;
    }
    let i = grouped.by_area.len();
    grouped
        .by_area
        .push((area_uuid.clone(), AreaTaskGroup::default()));
    area_pos.insert(area_uuid.clone(), i);
    i
}

fn ensure_project_group(
    grouped: &mut Vec<(ThingsId, Vec<usize>)>,
    project_pos: &mut HashMap<ThingsId, usize>,
    project_uuid: &ThingsId,
) -> usize {
    if let Some(i) = project_pos.get(project_uuid).copied() {
        return i;
    }
    let i = grouped.len();
    grouped.push((project_uuid.clone(), Vec::new()));
    project_pos.insert(project_uuid.clone(), i);
    i
}

fn group_items(items: &[Task], store: &ThingsStore) -> Grouped {
    let mut grouped = Grouped::default();
    let mut project_only_pos: HashMap<ThingsId, usize> = HashMap::new();
    let mut by_area_pos: HashMap<ThingsId, usize> = HashMap::new();

    for (idx, task) in items.iter().enumerate() {
        let project_uuid = store.effective_project_uuid(task);
        let area_uuid = store.effective_area_uuid(task);

        match (project_uuid, area_uuid) {
            (Some(project_uuid), Some(area_uuid)) => {
                let area_idx = ensure_area_group(&mut grouped, &mut by_area_pos, &area_uuid);
                let area_group = &mut grouped.by_area[area_idx].1;
                let project_idx = ensure_project_group(
                    &mut area_group.projects,
                    &mut area_group.project_pos,
                    &project_uuid,
                );
                area_group.projects[project_idx].1.push(idx);
            }
            (Some(project_uuid), None) => {
                let project_idx = ensure_project_group(
                    &mut grouped.project_only,
                    &mut project_only_pos,
                    &project_uuid,
                );
                grouped.project_only[project_idx].1.push(idx);
            }
            (None, Some(area_uuid)) => {
                let area_idx = ensure_area_group(&mut grouped, &mut by_area_pos, &area_uuid);
                grouped.by_area[area_idx].1.tasks.push(idx);
            }
            (None, None) => grouped.unscoped.push(idx),
        }
    }

    grouped
}

fn id_prefix_len(store: &ThingsStore, items: &[Task], grouped: &Grouped) -> usize {
    let mut ids: Vec<ThingsId> = items.iter().map(|t| t.uuid.clone()).collect();
    for (project_uuid, _) in &grouped.project_only {
        ids.push(project_uuid.clone());
    }
    for (area_uuid, area_group) in &grouped.by_area {
        ids.push(area_uuid.clone());
        for (project_uuid, _) in &area_group.projects {
            ids.push(project_uuid.clone());
        }
    }
    store.unique_prefix_length(&ids)
}

fn limited(tasks: &[usize]) -> (&[usize], usize) {
    let shown = tasks.len().min(MAX_GROUP_ITEMS);
    (&tasks[..shown], tasks.len().saturating_sub(shown))
}

#[derive(Default, Props)]
pub struct AnytimeViewProps<'a> {
    pub items: Option<&'a Vec<Task>>,
    pub detailed: bool,
}

#[component]
pub fn AnytimeView<'a>(hooks: Hooks, props: &AnytimeViewProps<'a>) -> impl Into<AnyElement<'a>> {
    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let Some(items) = props.items else {
        return element! { Text(content: "") }.into_any();
    };

    let content: AnyElement<'a> = {
        if items.is_empty() {
            element! { Text(content: "Anytime is empty.", wrap: TextWrap::NoWrap) }.into_any()
        } else {
            let grouped = group_items(items, store.as_ref());
            let prefix_len = id_prefix_len(store.as_ref(), items, &grouped);
            let options = TaskOptions {
                detailed: props.detailed,
                show_project: false,
                show_area: false,
                show_today_markers: true,
                show_staged_today_marker: false,
            };

            struct BlockSpec<'b> {
                header: Option<TaskGroupHeader>,
                items: Vec<&'b Task>,
                indent_under_header: u16,
                hidden_count: usize,
                extra_left: u32,
            }

            enum Entry<'b> {
                Sep,
                Block(BlockSpec<'b>),
            }

            let mut blocks: Vec<AnyElement<'a>> = Vec::new();
            let mut entries: Vec<Entry<'a>> = Vec::new();
            let mut first = true;

            if !grouped.unscoped.is_empty() {
                entries.push(Entry::Block(BlockSpec {
                    header: None,
                    items: grouped.unscoped.iter().map(|&i| &items[i]).collect(),
                    indent_under_header: 0u16,
                    hidden_count: 0usize,
                    extra_left: 0,
                }));
                first = false;
            }

            for (project_uuid, tasks) in &grouped.project_only {
                if !first {
                    entries.push(Entry::Sep);
                }
                let (shown, hidden) = limited(tasks);
                entries.push(Entry::Block(BlockSpec {
                    header: Some(TaskGroupHeader::Project {
                        project_uuid: project_uuid.clone(),
                        title: store.resolve_project_title(project_uuid),
                        id_prefix_len: prefix_len,
                    }),
                    items: shown.iter().map(|&i| &items[i]).collect(),
                    indent_under_header: 2u16,
                    hidden_count: hidden,
                    extra_left: 0,
                }));
                first = false;
            }

            for (area_uuid, area_group) in &grouped.by_area {
                if !first {
                    entries.push(Entry::Sep);
                }

                let (shown_area_tasks, hidden_area_tasks) = limited(&area_group.tasks);

                entries.push(Entry::Block(BlockSpec {
                    header: Some(TaskGroupHeader::Area {
                        area_uuid: area_uuid.clone(),
                        title: store.resolve_area_title(area_uuid),
                        id_prefix_len: prefix_len,
                    }),
                    items: shown_area_tasks.iter().map(|&i| &items[i]).collect(),
                    indent_under_header: 2u16,
                    hidden_count: hidden_area_tasks,
                    extra_left: 0,
                }));

                for (project_uuid, project_tasks) in &area_group.projects {
                    let (shown, hidden) = limited(project_tasks);
                    entries.push(Entry::Block(BlockSpec {
                        header: Some(TaskGroupHeader::Project {
                            project_uuid: project_uuid.clone(),
                            title: store.resolve_project_title(project_uuid),
                            id_prefix_len: prefix_len,
                        }),
                        items: shown.iter().map(|&i| &items[i]).collect(),
                        indent_under_header: 2u16,
                        hidden_count: hidden,
                        extra_left: 2,
                    }));
                }

                first = false;
            }

            for entry in entries {
                match entry {
                    Entry::Sep => {
                        blocks.push(
                            element! { Text(content: "", wrap: TextWrap::NoWrap) }.into_any(),
                        );
                    }
                    Entry::Block(spec) => {
                        let block = element! {
                            TaskGroup(
                                header: spec.header,
                                items: spec.items,
                                id_prefix_len: prefix_len,
                                options: options,
                                indent_under_header: spec.indent_under_header,
                                hidden_count: spec.hidden_count,
                            )
                        }
                        .into_any();

                        if spec.extra_left > 0 {
                            blocks.push(
                                element! {
                                    View(padding_left: spec.extra_left) { #(Some(block)) }
                                }
                                .into_any(),
                            );
                        } else {
                            blocks.push(block);
                        }
                    }
                }
            }

            element! {
                    View(flex_direction: FlexDirection::Column) {
                        Text(content: format!("{} Anytime  ({} tasks)", ICONS.anytime, items.len()), wrap: TextWrap::NoWrap)
                        Text(content: "", wrap: TextWrap::NoWrap)
                        View(flex_direction: FlexDirection::Column, padding_left: LIST_INDENT) {
                            #(blocks)
                        }
                    }
                }
                .into_any()
        }
    };

    content
}
