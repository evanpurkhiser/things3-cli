use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use clap::{ArgGroup, Args};
use iocraft::prelude::*;

use crate::{
    app::Cli,
    arg_types::IdentifierToken,
    commands::{Command, DetailedArgs},
    common::resolve_single_tag,
    ids::ThingsId,
    store::{Task, ThingsStore},
    ui::{
        render_element_to_string,
        views::find::{FindRow, FindView},
    },
    wire::task::{TaskStart, TaskStatus},
};

#[derive(Debug, Clone, Copy)]
struct MatchResult {
    matched: bool,
    checklist_only: bool,
}

impl MatchResult {
    fn no() -> Self {
        Self {
            matched: false,
            checklist_only: false,
        }
    }

    fn yes(checklist_only: bool) -> Self {
        Self {
            matched: true,
            checklist_only,
        }
    }
}

fn matches_project_filter(
    filter: &IdentifierToken,
    project_uuid: &str,
    project_title_lower: &str,
) -> bool {
    let token = filter.as_str();
    let lowered = token.to_ascii_lowercase();
    project_uuid.starts_with(token) || project_title_lower.contains(&lowered)
}

fn matches_area_filter(filter: &IdentifierToken, area_uuid: &str, area_title_lower: &str) -> bool {
    let token = filter.as_str();
    let lowered = token.to_ascii_lowercase();
    area_uuid.starts_with(token) || area_title_lower.contains(&lowered)
}

#[derive(Debug, Default, Args)]
#[command(about = "Search and filter tasks.")]
#[command(
    after_help = "Date filter syntax:  --deadline OP DATE\n  OP is one of: >  <  >=  <=  =\n  DATE is YYYY-MM-DD or a keyword: today, tomorrow, yesterday\n\n  Examples:\n    --deadline \"<today\"          overdue tasks\n    --deadline \">=2026-01-01\"    deadline on or after date\n    --created \">=2026-01-01\" --created \"<=2026-03-31\"   date range"
)]
#[command(group(ArgGroup::new("status").args(["incomplete", "completed", "canceled", "any_status"]).multiple(false)))]
#[command(group(ArgGroup::new("deadline_presence").args(["has_deadline", "no_deadline"]).multiple(false)))]
pub struct FindArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
    #[arg(help = "Case-insensitive substring to match against task title")]
    pub query: Option<String>,
    #[arg(long, short = 'i', help = "Only incomplete tasks (default)")]
    pub incomplete: bool,
    #[arg(long, short = 'n', help = "Also search query against note text")]
    pub notes: bool,
    #[arg(
        long,
        short = 'k',
        help = "Also search query against checklist item titles; implies --detailed for checklist-only matches"
    )]
    pub checklists: bool,
    #[arg(long, short = 'c', help = "Only completed tasks")]
    pub completed: bool,
    #[arg(long, short = 'x', help = "Only canceled tasks")]
    pub canceled: bool,
    #[arg(
        long = "any-status",
        short = 'A',
        help = "Match tasks regardless of status"
    )]
    pub any_status: bool,
    #[arg(
        long = "tag",
        short = 't',
        value_name = "TAG",
        help = "Has this tag (title or UUID prefix); repeatable, OR logic"
    )]
    tag_filters: Vec<IdentifierToken>,
    #[arg(
        long = "project",
        short = 'p',
        value_name = "PROJECT",
        help = "In this project (title substring or UUID prefix); repeatable, OR logic"
    )]
    project_filters: Vec<IdentifierToken>,
    #[arg(
        long = "area",
        short = 'a',
        value_name = "AREA",
        help = "In this area (title substring or UUID prefix); repeatable, OR logic"
    )]
    area_filters: Vec<IdentifierToken>,
    #[arg(long, short = 'I', help = "In Inbox view")]
    pub inbox: bool,
    #[arg(long, short = 'T', help = "In Today view")]
    pub today: bool,
    #[arg(long, short = 's', help = "In Someday")]
    pub someday: bool,
    #[arg(long, short = 'e', help = "Evening flag set")]
    pub evening: bool,
    #[arg(long = "has-deadline", short = 'H', help = "Has any deadline set")]
    pub has_deadline: bool,
    #[arg(long = "no-deadline", short = 'N', help = "No deadline set")]
    pub no_deadline: bool,
    #[arg(long, short = 'r', help = "Only recurring tasks")]
    pub recurring: bool,
    #[arg(
        long,
        short = 'l',
        value_name = "EXPR",
        help = "Deadline filter, e.g. '<today' or '>=2026-04-01' (repeatable for range)"
    )]
    pub deadline: Vec<String>,
    #[arg(
        long,
        short = 'S',
        value_name = "EXPR",
        help = "Scheduled start date filter (repeatable)"
    )]
    pub scheduled: Vec<String>,
    #[arg(
        long,
        short = 'C',
        value_name = "EXPR",
        help = "Creation date filter (repeatable)"
    )]
    pub created: Vec<String>,
    #[arg(
        long = "completed-on",
        short = 'o',
        value_name = "EXPR",
        help = "Completion date filter; implies --completed (repeatable)"
    )]
    pub completed_on: Vec<String>,
}

impl Command for FindArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
        let today = ctx.today();

        for (flag, exprs) in [
            ("--deadline", &self.deadline),
            ("--scheduled", &self.scheduled),
            ("--created", &self.created),
            ("--completed-on", &self.completed_on),
        ] {
            for expr in exprs {
                if let Err(err) = parse_date_expr(expr, flag, &today) {
                    eprintln!("{err}");
                    return Ok(());
                }
            }
        }

        let mut resolved_tag_uuids = Vec::new();
        for tag_filter in &self.tag_filters {
            let (tag, err) = resolve_single_tag(&store, tag_filter.as_str());
            if !err.is_empty() {
                eprintln!("{err}");
                return Ok(());
            }
            if let Some(tag) = tag {
                resolved_tag_uuids.push(tag.uuid);
            }
        }

        let mut matched: Vec<(Task, MatchResult)> = store
            .tasks_by_uuid
            .values()
            .filter_map(|task| {
                let result = matches(task, &store, self, &resolved_tag_uuids, &today);
                if result.matched {
                    Some((task.clone(), result))
                } else {
                    None
                }
            })
            .collect();

        matched.sort_by(|(a, _), (b, _)| {
            let a_proj = if a.is_project() { 0 } else { 1 };
            let b_proj = if b.is_project() { 0 } else { 1 };
            (a_proj, a.index, &a.uuid).cmp(&(b_proj, b.index, &b.uuid))
        });

        let rows = matched
            .iter()
            .map(|(task, result)| FindRow {
                task,
                force_detailed: result.checklist_only,
            })
            .collect::<Vec<_>>();

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    FindView(rows, detailed: self.detailed.detailed)
                }
            }
        };
        let rendered = render_element_to_string(&mut ui, cli.no_color);
        writeln!(out, "{}", rendered)?;

        Ok(())
    }
}

fn parse_date_value(
    value: &str,
    flag: &str,
    today: &DateTime<Utc>,
) -> Result<DateTime<Utc>, String> {
    let lowered = value.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "today" => Ok(*today),
        "tomorrow" => Ok(*today + Duration::days(1)),
        "yesterday" => Ok(*today - Duration::days(1)),
        _ => {
            let parsed = NaiveDate::parse_from_str(&lowered, "%Y-%m-%d").map_err(|_| {
                format!(
                    "Invalid date for {flag}: {value:?}. Expected YYYY-MM-DD, 'today', 'tomorrow', or 'yesterday'."
                )
            })?;
            let ndt = parsed.and_hms_opt(0, 0, 0).ok_or_else(|| {
                format!(
                    "Invalid date for {flag}: {value:?}. Expected YYYY-MM-DD, 'today', 'tomorrow', or 'yesterday'."
                )
            })?;
            Ok(Utc.from_utc_datetime(&ndt))
        }
    }
}

fn parse_date_expr(
    raw: &str,
    flag: &str,
    today: &DateTime<Utc>,
) -> Result<(&'static str, DateTime<Utc>), String> {
    let value = raw.trim();
    let (op, date_part) = if let Some(rest) = value.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = value.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = value.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = value.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = value.strip_prefix('=') {
        ("=", rest)
    } else {
        return Err(format!(
            "Invalid date expression for {flag}: {raw:?}. Expected an operator prefix: >, <, >=, <=, or =  (e.g. '<=2026-03-31')"
        ));
    };
    let date = parse_date_value(date_part, flag, today)?;
    Ok((op, date))
}

fn date_matches(field: Option<DateTime<Utc>>, op: &str, threshold: DateTime<Utc>) -> bool {
    let Some(field) = field else {
        return false;
    };

    let field_day = field
        .with_timezone(&Utc)
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|d| Utc.from_utc_datetime(&d));
    let threshold_day = threshold
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|d| Utc.from_utc_datetime(&d));
    let (Some(field_day), Some(threshold_day)) = (field_day, threshold_day) else {
        return false;
    };

    match op {
        ">" => field_day > threshold_day,
        "<" => field_day < threshold_day,
        ">=" => field_day >= threshold_day,
        "<=" => field_day <= threshold_day,
        "=" => field_day == threshold_day,
        _ => false,
    }
}

fn build_status_set(args: &FindArgs) -> Option<Vec<TaskStatus>> {
    if args.any_status {
        return None;
    }

    let mut chosen = Vec::new();
    if args.incomplete {
        chosen.push(TaskStatus::Incomplete);
    }
    if args.completed {
        chosen.push(TaskStatus::Completed);
    }
    if args.canceled {
        chosen.push(TaskStatus::Canceled);
    }

    if chosen.is_empty() && !args.completed_on.is_empty() {
        return Some(vec![TaskStatus::Completed]);
    }
    if chosen.is_empty() {
        return Some(vec![TaskStatus::Incomplete]);
    }
    Some(chosen)
}

fn matches(
    task: &Task,
    store: &ThingsStore,
    args: &FindArgs,
    resolved_tag_uuids: &[ThingsId],
    today: &DateTime<Utc>,
) -> MatchResult {
    if task.is_heading() || task.trashed || task.entity != "Task6" {
        return MatchResult::no();
    }

    if let Some(allowed_statuses) = build_status_set(args)
        && !allowed_statuses.contains(&task.status)
    {
        return MatchResult::no();
    }

    let mut checklist_only = false;
    if let Some(query) = &args.query {
        let q = query.to_ascii_lowercase();
        let title_match = task.title.to_ascii_lowercase().contains(&q);
        let notes_match = args.notes
            && task
                .notes
                .as_ref()
                .map(|n| n.to_ascii_lowercase().contains(&q))
                .unwrap_or(false);
        let checklist_match = args.checklists
            && task
                .checklist_items
                .iter()
                .any(|item| item.title.to_ascii_lowercase().contains(&q));

        if !title_match && !notes_match && !checklist_match {
            return MatchResult::no();
        }
        checklist_only = checklist_match && !title_match && !notes_match;
    }

    if !args.tag_filters.is_empty()
        && !resolved_tag_uuids
            .iter()
            .any(|tag_uuid| task.tags.iter().any(|task_tag| task_tag == tag_uuid))
    {
        return MatchResult::no();
    }

    if !args.project_filters.is_empty() {
        let Some(project_uuid) = store.effective_project_uuid(task) else {
            return MatchResult::no();
        };
        let Some(project) = store.get_task(&project_uuid.to_string()) else {
            return MatchResult::no();
        };

        let project_title = project.title.to_ascii_lowercase();
        let matched = args
            .project_filters
            .iter()
            .any(|f| matches_project_filter(f, &project_uuid.to_string(), &project_title));
        if !matched {
            return MatchResult::no();
        }
    }

    if !args.area_filters.is_empty() {
        let Some(area_uuid) = store.effective_area_uuid(task) else {
            return MatchResult::no();
        };
        let Some(area) = store.get_area(&area_uuid.to_string()) else {
            return MatchResult::no();
        };

        let area_title = area.title.to_ascii_lowercase();
        let matched = args
            .area_filters
            .iter()
            .any(|f| matches_area_filter(f, &area_uuid.to_string(), &area_title));
        if !matched {
            return MatchResult::no();
        }
    }

    if args.inbox && task.start != TaskStart::Inbox {
        return MatchResult::no();
    }
    if args.today && !task.is_today(today) {
        return MatchResult::no();
    }
    if args.someday && !task.in_someday() {
        return MatchResult::no();
    }
    if args.evening && !task.evening {
        return MatchResult::no();
    }
    if args.has_deadline && task.deadline.is_none() {
        return MatchResult::no();
    }
    if args.no_deadline && task.deadline.is_some() {
        return MatchResult::no();
    }
    if args.recurring && task.recurrence_rule.is_none() {
        return MatchResult::no();
    }

    for expr in &args.deadline {
        let Ok((op, threshold)) = parse_date_expr(expr, "--deadline", today) else {
            return MatchResult::no();
        };
        if !date_matches(task.deadline, op, threshold) {
            return MatchResult::no();
        }
    }
    for expr in &args.scheduled {
        let Ok((op, threshold)) = parse_date_expr(expr, "--scheduled", today) else {
            return MatchResult::no();
        };
        if !date_matches(task.start_date, op, threshold) {
            return MatchResult::no();
        }
    }
    for expr in &args.created {
        let Ok((op, threshold)) = parse_date_expr(expr, "--created", today) else {
            return MatchResult::no();
        };
        if !date_matches(task.creation_date, op, threshold) {
            return MatchResult::no();
        }
    }
    for expr in &args.completed_on {
        let Ok((op, threshold)) = parse_date_expr(expr, "--completed-on", today) else {
            return MatchResult::no();
        };
        if !date_matches(task.stop_date, op, threshold) {
            return MatchResult::no();
        }
    }

    if !args.any_status && !args.completed_on.is_empty() && task.status != TaskStatus::Completed {
        return MatchResult::no();
    }

    MatchResult::yes(checklist_only)
}
