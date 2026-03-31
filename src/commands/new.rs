use crate::app::Cli;
use crate::commands::Command;
use crate::common::{
    DIM, GREEN, ICONS, colored, day_to_timestamp, parse_day, resolve_tag_ids, task6_note,
};
use crate::store::Task;
use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use clap::Args;
use serde_json::json;
use std::cmp::Reverse;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
#[command(about = "Create a new task")]
pub struct NewArgs {
    /// Task title
    pub title: String,
    #[arg(
        long = "in",
        default_value = "inbox",
        help = "Container: inbox, clear, project UUID/prefix, or area UUID/prefix"
    )]
    pub in_target: String,
    #[arg(
        long,
        help = "Schedule: anytime, someday, today, evening, or YYYY-MM-DD"
    )]
    pub when: Option<String>,
    #[arg(long = "before", help = "Insert before this sibling task UUID/prefix")]
    pub before_id: Option<String>,
    #[arg(long = "after", help = "Insert after this sibling task UUID/prefix")]
    pub after_id: Option<String>,
    #[arg(long, default_value = "", help = "Task notes")]
    pub notes: String,
    #[arg(long, help = "Comma-separated tags (titles or UUID prefixes)")]
    pub tags: Option<String>,
    #[arg(long = "deadline", help = "Deadline date (YYYY-MM-DD)")]
    pub deadline_date: Option<String>,
}

fn base_new_props(title: &str, now: f64) -> TaskProps {
    TaskProps {
        title: title.to_string(),
        item_type: TaskType::Todo,
        status: TaskStatus::Incomplete,
        start_location: TaskStart::Inbox,
        creation_date: Some(now),
        modification_date: Some(now),
        conflict_overrides: Some(json!({"_t": "oo", "sn": {}})),
        ..Default::default()
    }
}

fn task_bucket(task: &Task, store: &crate::store::ThingsStore) -> Vec<String> {
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
}

fn props_bucket(props: &TaskProps) -> Vec<String> {
    if let Some(project_uuid) = props.parent_project_ids.first() {
        return vec![
            "task-project".to_string(),
            project_uuid.to_string(),
            String::new(),
        ];
    }
    if let Some(area_uuid) = props.area_ids.first() {
        let st = i32::from(props.start_location);
        return vec![
            "task-area".to_string(),
            area_uuid.to_string(),
            st.to_string(),
        ];
    }
    let st = i32::from(props.start_location);
    vec!["task-root".to_string(), st.to_string()]
}

fn plan_ix_insert(ordered: &[Task], insert_at: usize) -> (i32, Vec<(String, i32, String)>) {
    let prev_ix = if insert_at > 0 {
        Some(ordered[insert_at - 1].index)
    } else {
        None
    };
    let next_ix = if insert_at < ordered.len() {
        Some(ordered[insert_at].index)
    } else {
        None
    };
    let mut updates = Vec::new();

    if prev_ix.is_none() && next_ix.is_none() {
        return (0, updates);
    }
    if prev_ix.is_none() {
        return (next_ix.unwrap_or(0) - 1, updates);
    }
    if next_ix.is_none() {
        return (prev_ix.unwrap_or(0) + 1, updates);
    }
    if prev_ix.unwrap_or(0) + 1 < next_ix.unwrap_or(0) {
        return ((prev_ix.unwrap_or(0) + next_ix.unwrap_or(0)) / 2, updates);
    }

    let stride = 1024;
    let mut new_index = stride;
    let mut idx = 1;
    for i in 0..=ordered.len() {
        let target_ix = idx * stride;
        if i == insert_at {
            new_index = target_ix;
            idx += 1;
            continue;
        }
        let source_idx = if i < insert_at { i } else { i - 1 };
        if source_idx < ordered.len() {
            let entry = &ordered[source_idx];
            if entry.index != target_ix {
                updates.push((entry.uuid.to_string(), target_ix, entry.entity.clone()));
            }
            idx += 1;
        }
    }
    (new_index, updates)
}

#[derive(Debug, Clone)]
struct NewPlan {
    new_uuid: String,
    changes: BTreeMap<String, WireObject>,
    title: String,
}

fn build_new_plan(
    args: &NewArgs,
    store: &crate::store::ThingsStore,
    now: f64,
    today_ts: i64,
    next_id: &mut dyn FnMut() -> String,
) -> std::result::Result<NewPlan, String> {
    let today = Utc
        .timestamp_opt(today_ts, 0)
        .single()
        .unwrap_or_else(Utc::now)
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|d| Utc.from_utc_datetime(&d))
        .unwrap_or_else(Utc::now);
    let title = args.title.trim();
    if title.is_empty() {
        return Err("Task title cannot be empty.".to_string());
    }

    let mut props = base_new_props(title, now);
    if !args.notes.is_empty() {
        props.notes = Some(task6_note(&args.notes));
    }

    let anchor_id = args.before_id.as_ref().or(args.after_id.as_ref());
    let mut anchor: Option<Task> = None;
    if let Some(anchor_id) = anchor_id {
        let (task, err, _ambiguous) = store.resolve_task_identifier(anchor_id);
        if task.is_none() {
            return Err(err);
        }
        anchor = task;
    }

    let in_target = args.in_target.trim();
    if !in_target.eq_ignore_ascii_case("inbox") {
        let (project, _, _) = store.resolve_mark_identifier(in_target);
        let (area, _, _) = store.resolve_area_identifier(in_target);
        let project_uuid = project.as_ref().and_then(|p| {
            if p.is_project() {
                Some(p.uuid.clone())
            } else {
                None
            }
        });
        let area_uuid = area.map(|a| a.uuid);

        if project_uuid.is_some() && area_uuid.is_some() {
            return Err(format!(
                "Ambiguous --in target '{}' (matches project and area).",
                in_target
            ));
        }

        if project.is_some() && project_uuid.is_none() {
            return Err("--in target must be inbox, a project ID, or an area ID.".to_string());
        }

        if let Some(project_uuid) = project_uuid {
            props.parent_project_ids = vec![project_uuid.into()];
            props.start_location = TaskStart::Anytime;
        } else if let Some(area_uuid) = area_uuid {
            props.area_ids = vec![area_uuid.into()];
            props.start_location = TaskStart::Anytime;
        } else {
            return Err(format!("Container not found: {}", in_target));
        }
    }

    if let Some(when_raw) = &args.when {
        let when = when_raw.trim();
        if when.eq_ignore_ascii_case("anytime") {
            props.start_location = TaskStart::Anytime;
            props.scheduled_date = None;
        } else if when.eq_ignore_ascii_case("someday") {
            props.start_location = TaskStart::Someday;
            props.scheduled_date = None;
        } else if when.eq_ignore_ascii_case("today") {
            props.start_location = TaskStart::Anytime;
            props.scheduled_date = Some(today_ts);
            props.today_index_reference = Some(today_ts);
        } else {
            let parsed = match parse_day(Some(when), "--when") {
                Ok(Some(day)) => day,
                Ok(None) => {
                    return Err(
                        "--when requires anytime, someday, today, or YYYY-MM-DD".to_string()
                    );
                }
                Err(err) => return Err(err),
            };
            let day_ts = day_to_timestamp(parsed);
            props.start_location = TaskStart::Someday;
            props.scheduled_date = Some(day_ts);
            props.today_index_reference = Some(day_ts);
        }
    }

    if let Some(tags) = &args.tags {
        let (tag_ids, tag_err) = resolve_tag_ids(store, tags);
        if !tag_err.is_empty() {
            return Err(tag_err);
        }
        props.tag_ids = tag_ids;
    }

    if let Some(deadline_date) = &args.deadline_date {
        let parsed = match parse_day(Some(deadline_date), "--deadline") {
            Ok(Some(day)) => day,
            Ok(None) => return Err("--deadline requires YYYY-MM-DD".to_string()),
            Err(err) => return Err(err),
        };
        props.deadline = Some(day_to_timestamp(parsed) as i64);
    }

    let anchor_is_today = anchor
        .as_ref()
        .map(|a| a.start == TaskStart::Anytime && (a.is_today(&today) || a.evening))
        .unwrap_or(false);
    let target_bucket = props_bucket(&props);

    if let Some(anchor) = &anchor
        && !anchor_is_today
        && task_bucket(anchor, store) != target_bucket
    {
        return Err(
            "Cannot place new task relative to an item in a different container/list.".to_string(),
        );
    }

    let mut index_updates: Vec<(String, i32, String)> = Vec::new();
    let mut siblings = store
        .tasks_by_uuid
        .values()
        .filter(|t| {
            !t.trashed
                && t.status == TaskStatus::Incomplete
                && task_bucket(t, store) == target_bucket
        })
        .cloned()
        .collect::<Vec<_>>();
    siblings.sort_by_key(|t| (t.index, t.uuid.clone()));

    let mut structural_insert_at = 0usize;
    if let Some(anchor) = &anchor
        && task_bucket(anchor, store) == target_bucket
    {
        let anchor_pos = siblings.iter().position(|t| t.uuid == anchor.uuid);
        let Some(anchor_pos) = anchor_pos else {
            return Err("Anchor not found in target list.".to_string());
        };
        structural_insert_at = if args.before_id.is_some() {
            anchor_pos
        } else {
            anchor_pos + 1
        };
    }

    let (structural_ix, structural_updates) = plan_ix_insert(&siblings, structural_insert_at);
    props.sort_index = structural_ix;
    index_updates.extend(structural_updates);

    let new_is_today = props.start_location == TaskStart::Anytime
        && props.scheduled_date.map_or(false, |sr| sr <= today_ts);
    if new_is_today && anchor_is_today {
        let mut section_evening = if props.evening_bit != 0 { 1 } else { 0 };

        if anchor_is_today && let Some(anchor) = &anchor {
            section_evening = if anchor.evening { 1 } else { 0 };
            props.evening_bit = section_evening;
        }

        let mut today_siblings = store
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && t.start == TaskStart::Anytime
                    && (t.is_today(&today) || t.evening)
                    && (if t.evening { 1 } else { 0 }) == section_evening
            })
            .cloned()
            .collect::<Vec<_>>();
        today_siblings.sort_by_key(|task| {
            let tir = task.today_index_reference.unwrap_or(0);
            (Reverse(tir), task.today_index, Reverse(task.index))
        });

        let mut today_insert_at = 0usize;
        if anchor_is_today
            && let Some(anchor) = &anchor
            && (if anchor.evening { 1 } else { 0 }) == section_evening
            && let Some(anchor_pos) = today_siblings.iter().position(|t| t.uuid == anchor.uuid)
        {
            today_insert_at = if args.before_id.is_some() {
                anchor_pos
            } else {
                anchor_pos + 1
            };
        }

        let prev_today = if today_insert_at > 0 {
            today_siblings.get(today_insert_at - 1)
        } else {
            None
        };
        let next_today = today_siblings.get(today_insert_at);

        if let Some(next_today) = next_today {
            let next_tir = next_today.today_index_reference.unwrap_or(today_ts);
            props.today_index_reference = Some(next_tir);
            props.today_sort_index = next_today.today_index - 1;
        } else if let Some(prev_today) = prev_today {
            let prev_tir = prev_today.today_index_reference.unwrap_or(today_ts);
            props.today_index_reference = Some(prev_tir);
            props.today_sort_index = prev_today.today_index + 1;
        } else {
            props.today_index_reference = Some(today_ts);
            props.today_sort_index = 0;
        }
    }

    let new_uuid = next_id();

    let mut changes = BTreeMap::new();
    changes.insert(
        new_uuid.clone(),
        WireObject::create(EntityType::Task6, props.clone()),
    );

    for (task_uuid, task_index, task_entity) in index_updates {
        use crate::wire::task::TaskPatch;
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
    }

    Ok(NewPlan {
        new_uuid,
        changes,
        title: title.to_string(),
    })
}

impl Command for NewArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let now = ctx.now_timestamp();
        let today = ctx.today_timestamp();
        let mut id_gen = || ctx.next_id();
        let plan = match build_new_plan(self, &store, now, today, &mut id_gen) {
            Ok(plan) => plan,
            Err(err) => {
                eprintln!("{err}");
                return Ok(());
            }
        };

        if let Err(e) = ctx.commit_changes(plan.changes, None) {
            eprintln!("Failed to create task: {e}");
            return Ok(());
        }

        writeln!(
            out,
            "{} {}  {}",
            colored(&format!("{} Created", ICONS.done), &[GREEN], cli.no_color),
            plan.title,
            colored(&plan.new_uuid, &[DIM], cli.no_color)
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ThingsStore, fold_items};
    use crate::wire::area::AreaProps;
    use crate::wire::tags::TagProps;
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
    use serde_json::json;

    const NOW: f64 = 1_700_000_000.0;
    const NEW_UUID: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const INBOX_ANCHOR_UUID: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const INBOX_OTHER_UUID: &str = "KGvAPpMrzHAKMdgMiERP1V";
    const PROJECT_UUID: &str = "JFdhhhp37fpryAKu8UXwzK";
    const AREA_UUID: &str = "74rgJf6Qh9wYp2TcVk8mNB";
    const TAG_A_UUID: &str = "By8mN2qRk5Wv7Xc9Dt3HpL";
    const TAG_B_UUID: &str = "Cv9nP3sTk6Xw8Yd4Eu5JqM";
    const TODAY: i64 = 1_700_000_000;

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn task(
        uuid: &str,
        title: &str,
        st: i32,
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
                    status: TaskStatus::Incomplete,
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

    fn project(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Project,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Anytime,
                    sort_index: 0,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn area(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Area3,
                AreaProps {
                    title: title.to_string(),
                    sort_index: 0,
                    ..Default::default()
                },
            ),
        )
    }

    fn tag(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Tag4,
                TagProps {
                    title: title.to_string(),
                    sort_index: 0,
                    ..Default::default()
                },
            ),
        )
    }

    #[test]
    fn new_payload_parity_cases() {
        let mut id_gen = || NEW_UUID.to_string();

        let bare = build_new_plan(
            &NewArgs {
                title: "Ship release".to_string(),
                in_target: "inbox".to_string(),
                when: None,
                before_id: None,
                after_id: None,
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &build_store(vec![]),
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect("bare");
        let bare_json = serde_json::to_value(bare.changes).expect("to value");
        assert_eq!(bare_json[NEW_UUID]["t"], json!(0));
        assert_eq!(bare_json[NEW_UUID]["e"], json!("Task6"));
        assert_eq!(bare_json[NEW_UUID]["p"]["tt"], json!("Ship release"));
        assert_eq!(bare_json[NEW_UUID]["p"]["st"], json!(0));
        assert_eq!(bare_json[NEW_UUID]["p"]["cd"], json!(NOW));
        assert_eq!(bare_json[NEW_UUID]["p"]["md"], json!(NOW));

        let when_today = build_new_plan(
            &NewArgs {
                title: "Task today".to_string(),
                in_target: "inbox".to_string(),
                when: Some("today".to_string()),
                before_id: None,
                after_id: None,
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &build_store(vec![]),
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect("today");
        let p = &serde_json::to_value(when_today.changes).expect("to value")[NEW_UUID]["p"];
        assert_eq!(p["st"], json!(1));
        assert_eq!(p["sr"], json!(TODAY));
        assert_eq!(p["tir"], json!(TODAY));

        let full_store = build_store(vec![
            project(PROJECT_UUID, "Roadmap"),
            area(AREA_UUID, "Work"),
            tag(TAG_A_UUID, "urgent"),
            tag(TAG_B_UUID, "backend"),
        ]);
        let in_project = build_new_plan(
            &NewArgs {
                title: "Project task".to_string(),
                in_target: PROJECT_UUID.to_string(),
                when: None,
                before_id: None,
                after_id: None,
                notes: "line one".to_string(),
                tags: Some("urgent,backend".to_string()),
                deadline_date: Some("2032-05-06".to_string()),
            },
            &full_store,
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect("in project");
        let p = &serde_json::to_value(in_project.changes).expect("to value")[NEW_UUID]["p"];
        let deadline_ts = day_to_timestamp(
            parse_day(Some("2032-05-06"), "--deadline")
                .expect("parse")
                .expect("day"),
        );
        assert_eq!(p["pr"], json!([PROJECT_UUID]));
        assert_eq!(p["st"], json!(1));
        assert_eq!(p["tg"], json!([TAG_A_UUID, TAG_B_UUID]));
        assert_eq!(p["dd"], json!(deadline_ts));
    }

    #[test]
    fn new_after_gap_and_rebalance() {
        let mut id_gen = || NEW_UUID.to_string();
        let gap_store = build_store(vec![
            task(INBOX_ANCHOR_UUID, "Anchor", 0, 1024, None, None, 0),
            task(INBOX_OTHER_UUID, "Other", 0, 2048, None, None, 0),
        ]);
        let gap = build_new_plan(
            &NewArgs {
                title: "Inserted".to_string(),
                in_target: "inbox".to_string(),
                when: None,
                before_id: None,
                after_id: Some(INBOX_ANCHOR_UUID.to_string()),
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &gap_store,
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect("gap");
        assert_eq!(
            serde_json::to_value(gap.changes).expect("to value")[NEW_UUID]["p"]["ix"],
            json!(1536)
        );

        let rebalance_store = build_store(vec![
            task(INBOX_ANCHOR_UUID, "Anchor", 0, 1024, None, None, 0),
            task(INBOX_OTHER_UUID, "Other", 0, 1025, None, None, 0),
        ]);
        let rebalance = build_new_plan(
            &NewArgs {
                title: "Inserted".to_string(),
                in_target: "inbox".to_string(),
                when: None,
                before_id: None,
                after_id: Some(INBOX_ANCHOR_UUID.to_string()),
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &rebalance_store,
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect("rebalance");
        let rb = serde_json::to_value(rebalance.changes).expect("to value");
        assert_eq!(rb[NEW_UUID]["p"]["ix"], json!(2048));
        assert_eq!(rb[INBOX_OTHER_UUID]["p"], json!({"ix":3072,"md":NOW}));
    }

    #[test]
    fn new_rejections() {
        let mut id_gen = || NEW_UUID.to_string();
        let empty_title = build_new_plan(
            &NewArgs {
                title: "   ".to_string(),
                in_target: "inbox".to_string(),
                when: None,
                before_id: None,
                after_id: None,
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &build_store(vec![]),
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect_err("empty title");
        assert_eq!(empty_title, "Task title cannot be empty.");

        let unknown_container = build_new_plan(
            &NewArgs {
                title: "Ship".to_string(),
                in_target: "nope".to_string(),
                when: None,
                before_id: None,
                after_id: None,
                notes: String::new(),
                tags: None,
                deadline_date: None,
            },
            &build_store(vec![]),
            NOW,
            TODAY,
            &mut id_gen,
        )
        .expect_err("unknown container");
        assert_eq!(unknown_container, "Container not found: nope");
    }
}
