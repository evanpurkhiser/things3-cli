use std::{cmp::Ordering, collections::BTreeMap};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use clap::Args;

use crate::{
    app::Cli,
    commands::Command,
    common::{DIM, GREEN, ICONS, colored},
    wire::{
        task::{TaskPatch, TaskStart, TaskStatus},
        wire_object::{EntityType, WireObject},
    },
};

#[derive(Debug, Args)]
#[command(about = "Reorder item relative to another item")]
pub struct ReorderArgs {
    /// Item UUID (or unique UUID prefix)
    pub item_id: String,
    #[arg(long, short = 'b', help = "Anchor item UUID/prefix to place before")]
    pub before_id: Option<String>,
    #[arg(long, short = 'a', help = "Anchor item UUID/prefix to place after")]
    pub after_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ReorderCommit {
    changes: BTreeMap<String, WireObject>,
    ancestor_index: Option<i64>,
}

#[derive(Debug, Clone)]
struct ReorderPlan {
    item: crate::store::Task,
    commits: Vec<ReorderCommit>,
    reorder_label: String,
}

fn build_reorder_plan(
    args: &ReorderArgs,
    store: &crate::store::ThingsStore,
    now: f64,
    today_ts: i64,
    initial_ancestor_index: Option<i64>,
) -> std::result::Result<ReorderPlan, String> {
    let today = Utc
        .timestamp_opt(today_ts, 0)
        .single()
        .unwrap_or_else(Utc::now)
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|d| Utc.from_utc_datetime(&d))
        .unwrap_or_else(Utc::now);
    let (item_opt, err, _) = store.resolve_task_identifier(&args.item_id);
    let Some(item) = item_opt else {
        return Err(err);
    };

    let anchor_id = args
        .before_id
        .as_ref()
        .or(args.after_id.as_ref())
        .cloned()
        .unwrap_or_default();
    let (anchor_opt, err, _) = store.resolve_task_identifier(&anchor_id);
    let Some(anchor) = anchor_opt else {
        return Err(err);
    };

    if item.uuid == anchor.uuid {
        return Err("Cannot reorder an item relative to itself.".to_string());
    }

    let is_today_orderable = |task: &crate::store::Task| {
        task.start == TaskStart::Anytime && (task.is_today(&today) || task.evening)
    };
    let is_today_reorder = is_today_orderable(&item) && is_today_orderable(&anchor);

    if is_today_reorder {
        let anchor_tir = anchor
            .today_index_reference
            .or_else(|| anchor.start_date.map(|d| d.timestamp()))
            .unwrap_or(today_ts);
        let new_ti = if args.before_id.is_some() {
            anchor.today_index - 1
        } else {
            anchor.today_index + 1
        };

        let sb = if item.evening != anchor.evening {
            Some(if anchor.evening { 1 } else { 0 })
        } else {
            None
        };
        let mut changes = BTreeMap::new();
        changes.insert(
            item.uuid.to_string(),
            WireObject::update(
                EntityType::from(item.entity.clone()),
                TaskPatch {
                    today_index_reference: Some(Some(anchor_tir)),
                    today_sort_index: Some(new_ti),
                    evening_bit: sb,
                    modification_date: Some(now),
                    ..Default::default()
                },
            ),
        );

        let reorder_label = if args.before_id.is_some() {
            format!(
                "(before={}, today_ref={}, today_index={})",
                anchor.title, anchor_tir, new_ti
            )
        } else {
            format!(
                "(after={}, today_ref={}, today_index={})",
                anchor.title, anchor_tir, new_ti
            )
        };

        return Ok(ReorderPlan {
            item,
            commits: vec![ReorderCommit {
                changes,
                ancestor_index: initial_ancestor_index,
            }],
            reorder_label,
        });
    }

    let bucket = |task: &crate::store::Task| -> Vec<String> {
        if task.is_heading() {
            return vec![
                "heading".to_string(),
                task.project
                    .clone()
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ];
        }
        if task.is_project() {
            return vec![
                "project".to_string(),
                task.area.clone().map(|v| v.to_string()).unwrap_or_default(),
            ];
        }
        if let Some(project_uuid) = store.effective_project_uuid(task) {
            return vec![
                "task-project".to_string(),
                project_uuid.to_string(),
                task.action_group
                    .clone()
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ];
        }
        if let Some(area_uuid) = store.effective_area_uuid(task) {
            return vec![
                "task-area".to_string(),
                area_uuid.to_string(),
                i32::from(task.start).to_string(),
            ];
        }
        vec!["task-root".to_string(), i32::from(task.start).to_string()]
    };

    let item_bucket = bucket(&item);
    let anchor_bucket = bucket(&anchor);
    if item_bucket != anchor_bucket {
        return Err("Cannot reorder across different containers/lists.".to_string());
    }

    let mut siblings = store
        .tasks_by_uuid
        .values()
        .filter(|t| !t.trashed && t.status == TaskStatus::Incomplete && bucket(t) == item_bucket)
        .cloned()
        .collect::<Vec<_>>();
    siblings.sort_by(|a, b| match a.index.cmp(&b.index) {
        Ordering::Equal => a.uuid.cmp(&b.uuid),
        other => other,
    });

    let by_uuid = siblings
        .iter()
        .map(|t| (t.uuid.clone(), t.clone()))
        .collect::<BTreeMap<_, _>>();
    if !by_uuid.contains_key(&item.uuid) || !by_uuid.contains_key(&anchor.uuid) {
        return Err("Cannot reorder item in the selected list.".to_string());
    }

    let mut order = siblings
        .into_iter()
        .filter(|t| t.uuid != item.uuid)
        .collect::<Vec<_>>();
    let anchor_pos = order.iter().position(|t| t.uuid == anchor.uuid);
    let Some(anchor_pos) = anchor_pos else {
        return Err("Anchor not found in reorder list.".to_string());
    };
    let insert_at = if args.before_id.is_some() {
        anchor_pos
    } else {
        anchor_pos + 1
    };
    order.insert(insert_at, item.clone());

    let moved_pos = order.iter().position(|t| t.uuid == item.uuid).unwrap_or(0);
    let prev_ix = if moved_pos > 0 {
        Some(order[moved_pos - 1].index)
    } else {
        None
    };
    let next_ix = if moved_pos + 1 < order.len() {
        Some(order[moved_pos + 1].index)
    } else {
        None
    };

    let mut index_updates: Vec<(String, i32, String)> = Vec::new();
    let new_index = if prev_ix.is_none() && next_ix.is_none() {
        0
    } else if prev_ix.is_none() {
        next_ix.unwrap_or(0) - 1
    } else if next_ix.is_none() {
        prev_ix.unwrap_or(0) + 1
    } else if prev_ix.unwrap_or(0) + 1 < next_ix.unwrap_or(0) {
        (prev_ix.unwrap_or(0) + next_ix.unwrap_or(0)) / 2
    } else {
        let stride = 1024;
        for (idx, task) in order.iter().enumerate() {
            let target_ix = (idx as i32 + 1) * stride;
            if task.index != target_ix {
                index_updates.push((task.uuid.to_string(), target_ix, task.entity.clone()));
            }
        }
        index_updates
            .iter()
            .find(|(uid, _, _)| uid == &item.uuid.to_string())
            .map(|(_, ix, _)| *ix)
            .unwrap_or(item.index)
    };

    if index_updates.is_empty() && new_index != item.index {
        index_updates.push((item.uuid.to_string(), new_index, item.entity.clone()));
    }

    let mut commits = Vec::new();
    let mut ancestor = initial_ancestor_index;
    for (task_uuid, task_index, task_entity) in index_updates {
        let mut changes = BTreeMap::new();
        changes.insert(
            task_uuid,
            WireObject::update(
                EntityType::from(task_entity),
                TaskPatch {
                    sort_index: Some(task_index),
                    modification_date: Some(now),
                    ..Default::default()
                },
            ),
        );
        commits.push(ReorderCommit {
            changes,
            ancestor_index: ancestor,
        });
        ancestor = ancestor.map(|v| v + 1).or(Some(1));
    }

    let reorder_label = if args.before_id.is_some() {
        format!("(before={}, index={})", anchor.title, new_index)
    } else {
        format!("(after={}, index={})", anchor.title, new_index)
    };

    Ok(ReorderPlan {
        item,
        commits,
        reorder_label,
    })
}

impl Command for ReorderArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let plan = match build_reorder_plan(
            self,
            &store,
            ctx.now_timestamp(),
            ctx.today_timestamp(),
            None,
        ) {
            Ok(plan) => plan,
            Err(err) => {
                eprintln!("{err}");
                return Ok(());
            }
        };

        for commit in plan.commits {
            if let Err(e) = ctx.commit_changes(commit.changes, commit.ancestor_index) {
                eprintln!("Failed to reorder item: {e}");
                return Ok(());
            }
        }

        writeln!(
            out,
            "{} {}  {} {}",
            colored(&format!("{} Reordered", ICONS.done), &[GREEN], cli.no_color),
            plan.item.title,
            colored(&plan.item.uuid, &[DIM], cli.no_color),
            colored(&plan.reorder_label, &[DIM], cli.no_color)
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        store::{ThingsStore, fold_items},
        wire::task::{TaskProps, TaskStart, TaskStatus, TaskType},
    };

    const NOW: f64 = 1_700_000_444.0;
    const TASK_A: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const TASK_B: &str = "KGvAPpMrzHAKMdgMiERP1V";
    const TASK_C: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const TODAY: i64 = 1_699_920_000; // 2023-11-14 00:00:00 UTC (midnight)

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    #[allow(clippy::too_many_arguments)]
    fn task(
        uuid: &str,
        title: &str,
        st: i32,
        ss: i32,
        ix: i32,
        sr: Option<i64>,
        tir: Option<i64>,
        ti: i32,
    ) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Todo,
                    status: TaskStatus::from(ss),
                    start_location: TaskStart::from(st),
                    sort_index: ix,
                    scheduled_date: sr,
                    today_index_reference: tir,
                    today_sort_index: ti,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    #[test]
    fn reorder_before_after_and_today_payloads() {
        let store = build_store(vec![
            task(TASK_A, "A", 0, 0, 1024, None, None, 0),
            task(TASK_B, "B", 0, 0, 2048, None, None, 0),
            task(TASK_C, "C", 0, 0, 3072, None, None, 0),
        ]);

        let before = build_reorder_plan(
            &ReorderArgs {
                item_id: TASK_C.to_string(),
                before_id: Some(TASK_B.to_string()),
                after_id: None,
            },
            &store,
            NOW,
            TODAY,
            None,
        )
        .expect("before plan");
        assert_eq!(before.commits.len(), 1);
        assert_eq!(
            serde_json::to_value(before.commits[0].changes.clone()).expect("to value"),
            json!({ TASK_C: {"t":1,"e":"Task6","p":{"ix":1536,"md":NOW}} })
        );

        let store_today = build_store(vec![
            task(TASK_A, "A", 1, 0, 100, Some(TODAY), Some(TODAY), 10),
            task(TASK_B, "B", 1, 0, 200, Some(TODAY), Some(TODAY), 20),
        ]);
        let today_plan = build_reorder_plan(
            &ReorderArgs {
                item_id: TASK_A.to_string(),
                before_id: None,
                after_id: Some(TASK_B.to_string()),
            },
            &store_today,
            NOW,
            TODAY,
            None,
        )
        .expect("today plan");
        assert_eq!(
            serde_json::to_value(today_plan.commits[0].changes.clone()).expect("to value"),
            json!({ TASK_A: {"t":1,"e":"Task6","p":{"tir":TODAY,"ti":21,"md":NOW}} })
        );
    }

    #[test]
    fn reorder_rebalance_and_errors() {
        let store = build_store(vec![
            task(TASK_A, "A", 0, 0, 1024, None, None, 0),
            task(TASK_B, "B", 0, 0, 1025, None, None, 0),
            task(TASK_C, "C", 0, 0, 1026, None, None, 0),
        ]);
        let rebalance = build_reorder_plan(
            &ReorderArgs {
                item_id: TASK_C.to_string(),
                before_id: None,
                after_id: Some(TASK_A.to_string()),
            },
            &store,
            NOW,
            TODAY,
            Some(50),
        )
        .expect("rebalance");
        assert_eq!(rebalance.commits.len(), 2);
        assert_eq!(rebalance.commits[0].ancestor_index, Some(50));
        assert_eq!(rebalance.commits[1].ancestor_index, Some(51));

        let err = build_reorder_plan(
            &ReorderArgs {
                item_id: TASK_A.to_string(),
                before_id: Some(TASK_A.to_string()),
                after_id: None,
            },
            &store,
            NOW,
            TODAY,
            None,
        )
        .expect_err("self reorder");
        assert_eq!(err, "Cannot reorder an item relative to itself.");
    }
}
