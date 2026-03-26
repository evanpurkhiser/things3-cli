use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{DIM, GREEN, ICONS, colored, day_to_timestamp, parse_day};
use crate::wire::{EntityType, OperationType, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct ScheduleArgs {
    pub task_id: String,
    #[arg(long)]
    pub when: Option<String>,
    #[arg(long = "deadline")]
    pub deadline_date: Option<String>,
    #[arg(long = "clear-deadline")]
    pub clear_deadline: bool,
}

fn now_day_ts() -> i64 {
    crate::common::today_utc().timestamp()
}

impl Command for ScheduleArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let store = cli.load_store()?;
        let (task_opt, err, _) = store.resolve_mark_identifier(&self.task_id);
        let Some(task) = task_opt else {
            eprintln!("{err}");
            return Ok(());
        };

        let mut update: BTreeMap<String, Value> = BTreeMap::new();
        let mut when_label: Option<String> = None;

        if let Some(when_raw) = &self.when {
            let when = when_raw.trim();
            let when_l = when.to_lowercase();
            if when_l == "anytime" {
                update.insert("st".to_string(), json!(1));
                update.insert("sr".to_string(), Value::Null);
                update.insert("tir".to_string(), Value::Null);
                update.insert("sb".to_string(), json!(0));
                when_label = Some("anytime".to_string());
            } else if when_l == "today" {
                let day_ts = now_day_ts();
                update.insert("st".to_string(), json!(1));
                update.insert("sr".to_string(), json!(day_ts));
                update.insert("tir".to_string(), json!(day_ts));
                update.insert("sb".to_string(), json!(0));
                when_label = Some("today".to_string());
            } else if when_l == "evening" {
                let day_ts = now_day_ts();
                update.insert("st".to_string(), json!(1));
                update.insert("sr".to_string(), json!(day_ts));
                update.insert("tir".to_string(), json!(day_ts));
                update.insert("sb".to_string(), json!(1));
                when_label = Some("evening".to_string());
            } else if when_l == "someday" {
                update.insert("st".to_string(), json!(2));
                update.insert("sr".to_string(), Value::Null);
                update.insert("tir".to_string(), Value::Null);
                update.insert("sb".to_string(), json!(0));
                when_label = Some("someday".to_string());
            } else {
                let when_day = match parse_day(Some(when), "--when") {
                    Ok(Some(day)) => day,
                    Ok(None) => return Ok(()),
                    Err(e) => {
                        eprintln!("{e}");
                        return Ok(());
                    }
                };
                let day_ts = day_to_timestamp(when_day);
                let today_ts = now_day_ts();
                if day_ts <= today_ts {
                    update.insert("st".to_string(), json!(1));
                    update.insert("sr".to_string(), json!(day_ts));
                    update.insert("tir".to_string(), json!(day_ts));
                    update.insert("sb".to_string(), json!(0));
                } else {
                    update.insert("st".to_string(), json!(2));
                    update.insert("sr".to_string(), json!(day_ts));
                    update.insert("tir".to_string(), json!(day_ts));
                    update.insert("sb".to_string(), json!(0));
                }
                when_label = Some(format!("when={when}"));
            }
        }

        if let Some(deadline) = &self.deadline_date {
            let day = match parse_day(Some(deadline), "--deadline") {
                Ok(Some(day)) => day,
                Ok(None) => return Ok(()),
                Err(e) => {
                    eprintln!("{e}");
                    return Ok(());
                }
            };
            update.insert("dd".to_string(), json!(day_to_timestamp(day)));
        }
        if self.clear_deadline {
            update.insert("dd".to_string(), Value::Null);
        }

        if update.is_empty() {
            eprintln!("No schedule changes requested.");
            return Ok(());
        }

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let _ = client.authenticate();

        let now = Utc::now().timestamp_millis() as f64 / 1000.0;
        update.insert("md".to_string(), json!(now));

        let mut changes = BTreeMap::new();
        changes.insert(
            task.uuid.clone(),
            WireObject {
                operation_type: OperationType::Update,
                entity_type: Some(EntityType::from(task.entity.clone())),
                properties: update.clone(),
            },
        );

        if let Err(e) = client.commit(changes, None) {
            eprintln!("Failed to schedule item: {e}");
            return Ok(());
        }

        let mut labels = Vec::new();
        if update.contains_key("st") {
            labels.push(when_label.unwrap_or_else(|| "when".to_string()));
        }
        if update.contains_key("dd") {
            if update.get("dd").is_some_and(Value::is_null) {
                labels.push("deadline=none".to_string());
            } else {
                labels.push(format!(
                    "deadline={}",
                    self.deadline_date.clone().unwrap_or_default()
                ));
            }
        }

        writeln!(
            out,
            "{} {}  {} {}",
            colored(&format!("{} Scheduled", ICONS.done), &[GREEN], cli.no_color),
            task.title,
            colored(&task.uuid, &[DIM], cli.no_color),
            colored(&format!("({})", labels.join(", ")), &[DIM], cli.no_color)
        )?;

        Ok(())
    }
}
