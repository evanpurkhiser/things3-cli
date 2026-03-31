use crate::common::ICONS;
use crate::store::{Task, ThingsStore};
use iocraft::prelude::*;
use std::sync::Arc;

#[derive(Default, Props)]
pub struct ProgressBadgeProps<'a> {
    pub project: Option<&'a Task>,
    pub show_count: bool,
    pub title: Option<String>,
    pub color: Option<Color>,
    pub weight: Weight,
}

#[component]
pub fn ProgressBadge<'a>(
    hooks: Hooks,
    props: &ProgressBadgeProps<'a>,
) -> impl Into<AnyElement<'a>> {
    let Some(project) = props.project else {
        return element!(Fragment).into_any();
    };

    let store = hooks.use_context::<Arc<ThingsStore>>().clone();
    let progress = project_progress(project, store.as_ref());
    let color = props.color.unwrap_or(Color::DarkGrey);
    let weight = props.weight;

    let title = if let Some(title) = &props.title {
        element!(Text(content: title.clone(), color, weight)).into_any()
    } else {
        element!(Fragment).into_any()
    };

    let count = if props.show_count {
        let content = format!("({}/{})", progress.done, progress.done + progress.total);
        element!(Text(content, color, weight)).into_any()
    } else {
        element!(Fragment).into_any()
    };

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            Text(content: progress.marker, color, weight)
            #(title)
            #(count)
        }
    }
    .into_any()
}

struct Progress {
    marker: &'static str,
    total: i32,
    done: i32,
}

fn project_progress(project: &Task, store: &ThingsStore) -> Progress {
    if project.in_someday() {
        return Progress {
            marker: ICONS.project_someday,
            total: 0,
            done: 0,
        };
    }

    let progress = store.project_progress(&project.uuid);
    let total = progress.total;
    let done = progress.done;

    if total == 0 || done == 0 {
        return Progress {
            marker: ICONS.progress_empty,
            total,
            done,
        };
    }

    if done == total {
        return Progress {
            marker: ICONS.progress_full,
            total,
            done,
        };
    }

    let ratio = done as f32 / total as f32;
    let marker = if ratio < (1.0 / 3.0) {
        ICONS.progress_quarter
    } else if ratio < (2.0 / 3.0) {
        ICONS.progress_half
    } else {
        ICONS.progress_three_quarter
    };

    Progress {
        marker,
        total,
        done,
    }
}
