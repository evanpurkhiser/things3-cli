use crate::app::Cli;
use crate::commands::Command;
use crate::common::{colored, day_to_timestamp, parse_day, DIM, GREEN, ICONS};
use crate::wire::task::{TaskPatch, TaskStart};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use clap::Args;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
#[command(about = "Set when and deadline")]
pub struct ScheduleArgs {
    /// Task UUID (or unique UUID prefix)
    pub task_id: String,
    #[arg(long, help = "When: anytime, today, evening, someday, or YYYY-MM-DD")]
    pub when: Option<String>,
    #[arg(long = "deadline", help = "Deadline date (YYYY-MM-DD)")]
    pub deadline_date: Option<String>,
    #[arg(long = "clear-deadline", help = "Clear deadline")]
    pub clear_deadline: bool,
}

#[derive(Debug, Clone)]
struct SchedulePlan {
    task: crate::store::Task,
    update: TaskPatch,
    labels: Vec<String>,
}

fn build_schedule_plan(
    args: &ScheduleArgs,
    store: &crate::store::ThingsStore,
    now: f64,
    today_ts: i64,
) -> std::result::Result<SchedulePlan, String> {
    let (task_opt, err, _) = store.resolve_mark_identifier(&args.task_id);
    let Some(task) = task_opt else {
        return Err(err);
    };

    let mut update = TaskPatch::default();
    let mut when_label: Option<String> = None;

    if let Some(when_raw) = &args.when {
        let when = when_raw.trim();
        let when_l = when.to_lowercase();
        if when_l == "anytime" {
            update.start_location = Some(TaskStart::Anytime);
            update.scheduled_date = Some(None);
            update.today_index_reference = Some(None);
            update.evening_bit = Some(0);
            when_label = Some("anytime".to_string());
        } else if when_l == "today" {
            update.start_location = Some(TaskStart::Anytime);
            update.scheduled_date = Some(Some(today_ts));
            update.today_index_reference = Some(Some(today_ts));
            update.evening_bit = Some(0);
            when_label = Some("today".to_string());
        } else if when_l == "evening" {
            update.start_location = Some(TaskStart::Anytime);
            update.scheduled_date = Some(Some(today_ts));
            update.today_index_reference = Some(Some(today_ts));
            update.evening_bit = Some(1);
            when_label = Some("evening".to_string());
        } else if when_l == "someday" {
            update.start_location = Some(TaskStart::Someday);
            update.scheduled_date = Some(None);
            update.today_index_reference = Some(None);
            update.evening_bit = Some(0);
            when_label = Some("someday".to_string());
        } else {
            let when_day = match parse_day(Some(when), "--when") {
                Ok(Some(day)) => day,
                Ok(None) => {
                    return Err(
                        "--when requires anytime, someday, today, or YYYY-MM-DD".to_string()
                    );
                }
                Err(e) => return Err(e),
            };
            let day_ts = day_to_timestamp(when_day);
            if day_ts <= today_ts {
                update.start_location = Some(TaskStart::Anytime);
            } else {
                update.start_location = Some(TaskStart::Someday);
            }
            update.scheduled_date = Some(Some(day_ts));
            update.today_index_reference = Some(Some(day_ts));
            update.evening_bit = Some(0);
            when_label = Some(format!("when={when}"));
        }
    }

    if let Some(deadline) = &args.deadline_date {
        let day = match parse_day(Some(deadline), "--deadline") {
            Ok(Some(day)) => day,
            Ok(None) => return Err("--deadline requires YYYY-MM-DD".to_string()),
            Err(e) => return Err(e),
        };
        update.deadline = Some(Some(day_to_timestamp(day) as f64));
    }
    if args.clear_deadline {
        update.deadline = Some(None);
    }

    if update.is_empty() {
        return Err("No schedule changes requested.".to_string());
    }

    update.modification_date = Some(now);

    let mut labels = Vec::new();
    if update.start_location.is_some() {
        labels.push(when_label.unwrap_or_else(|| "when".to_string()));
    }
    if update.deadline.is_some() {
        if update.deadline == Some(None) {
            labels.push("deadline=none".to_string());
        } else {
            labels.push(format!(
                "deadline={}",
                args.deadline_date.clone().unwrap_or_default()
            ));
        }
    }

    Ok(SchedulePlan {
        task,
        update,
        labels,
    })
}

impl Command for ScheduleArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let plan =
            match build_schedule_plan(self, &store, ctx.now_timestamp(), ctx.today_timestamp()) {
                Ok(plan) => plan,
                Err(err) => {
                    eprintln!("{err}");
                    return Ok(());
                }
            };

        let mut changes = BTreeMap::new();
        changes.insert(
            plan.task.uuid.to_string(),
            WireObject::update(
                EntityType::from(plan.task.entity.clone()),
                plan.update.clone(),
            ),
        );

        if let Err(e) = ctx.commit_changes(changes, None) {
            eprintln!("Failed to schedule item: {e}");
            return Ok(());
        }

        writeln!(
            out,
            "{} {}  {} {}",
            colored(&format!("{} Scheduled", ICONS.done), &[GREEN], cli.no_color),
            plan.task.title,
            colored(&plan.task.uuid, &[DIM], cli.no_color),
            colored(
                &format!("({})", plan.labels.join(", ")),
                &[DIM],
                cli.no_color
            )
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{fold_items, ThingsStore};
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
    use crate::wire::wire_object::WireItem;
    use crate::wire::wire_object::{EntityType, WireObject};
    use serde_json::json;

    const NOW: f64 = 1_700_000_333.0;
    const TASK_UUID: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const TODAY: i64 = 1_700_000_000;

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item: WireItem = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn task(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Todo,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Inbox,
                    sort_index: 0,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    #[test]
    fn schedule_when_variants_payloads() {
        let store = build_store(vec![task(TASK_UUID, "Schedule me")]);
        let future_ts = day_to_timestamp(
            parse_day(Some("2099-05-10"), "--when")
                .expect("parse")
                .expect("day"),
        );
        let cases = [
            (
                "today",
                json!({"st":1,"sr":TODAY,"tir":TODAY,"sb":0,"md":NOW}),
            ),
            (
                "someday",
                json!({"st":2,"sr":null,"tir":null,"sb":0,"md":NOW}),
            ),
            (
                "anytime",
                json!({"st":1,"sr":null,"tir":null,"sb":0,"md":NOW}),
            ),
            (
                "evening",
                json!({"st":1,"sr":TODAY,"tir":TODAY,"sb":1,"md":NOW}),
            ),
            (
                "2099-05-10",
                json!({"st":2,"sr":future_ts,"tir":future_ts,"sb":0,"md":NOW}),
            ),
        ];

        for (when, expected) in cases {
            let plan = build_schedule_plan(
                &ScheduleArgs {
                    task_id: TASK_UUID.to_string(),
                    when: Some(when.to_string()),
                    deadline_date: None,
                    clear_deadline: false,
                },
                &store,
                NOW,
                TODAY,
            )
            .expect("schedule plan");
            assert_eq!(
                serde_json::to_value(plan.update).expect("to value"),
                expected
            );
        }
    }

    #[test]
    fn schedule_deadline_and_clear_payloads() {
        let store = build_store(vec![task(TASK_UUID, "Schedule me")]);
        let deadline_ts = day_to_timestamp(
            parse_day(Some("2034-02-01"), "--deadline")
                .expect("parse")
                .expect("day"),
        );

        let deadline = build_schedule_plan(
            &ScheduleArgs {
                task_id: TASK_UUID.to_string(),
                when: None,
                deadline_date: Some("2034-02-01".to_string()),
                clear_deadline: false,
            },
            &store,
            NOW,
            TODAY,
        )
        .expect("deadline plan");
        assert_eq!(
            serde_json::to_value(deadline.update).expect("to value"),
            json!({"dd": deadline_ts as f64, "md": NOW})
        );

        let clear = build_schedule_plan(
            &ScheduleArgs {
                task_id: TASK_UUID.to_string(),
                when: None,
                deadline_date: None,
                clear_deadline: true,
            },
            &store,
            NOW,
            TODAY,
        )
        .expect("clear plan");
        assert_eq!(
            serde_json::to_value(clear.update).expect("to value"),
            json!({"dd": null, "md": NOW})
        );
    }

    #[test]
    fn schedule_rejections() {
        let store = build_store(vec![task(TASK_UUID, "A")]);
        let no_changes = build_schedule_plan(
            &ScheduleArgs {
                task_id: TASK_UUID.to_string(),
                when: None,
                deadline_date: None,
                clear_deadline: false,
            },
            &store,
            NOW,
            TODAY,
        )
        .expect_err("no changes");
        assert_eq!(no_changes, "No schedule changes requested.");

        let invalid_when = build_schedule_plan(
            &ScheduleArgs {
                task_id: TASK_UUID.to_string(),
                when: Some("2024-02-31".to_string()),
                deadline_date: None,
                clear_deadline: false,
            },
            &store,
            NOW,
            TODAY,
        )
        .expect_err("invalid when");
        assert_eq!(
            invalid_when,
            "Invalid --when date: 2024-02-31 (expected YYYY-MM-DD)"
        );
    }
}
