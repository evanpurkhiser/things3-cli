use crate::app::Cli;
use crate::arg_types::IdentifierToken;
use crate::commands::{Command, DetailedArgs};
use crate::common::{
    BOLD, CYAN, DIM, ICONS, colored, fmt_project_with_note, fmt_task_line, fmt_task_with_note,
    resolve_single_tag,
};
use crate::store::{Task, ThingsStore};
use crate::wire::{TaskStart, TaskStatus};
use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use clap::{ArgGroup, Args};

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

fn matches_project_filter(filter: &IdentifierToken, project_uuid: &str, project_title_lower: &str) -> bool {
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
#[command(group(ArgGroup::new("status").args(["incomplete", "completed", "canceled", "any_status"]).multiple(false)))]
#[command(group(ArgGroup::new("deadline_presence").args(["has_deadline", "no_deadline"]).multiple(false)))]
pub struct FindArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
    pub query: Option<String>,
    #[arg(long)]
    pub incomplete: bool,
    #[arg(long)]
    pub notes: bool,
    #[arg(long)]
    pub checklists: bool,
    #[arg(long)]
    pub completed: bool,
    #[arg(long)]
    pub canceled: bool,
    #[arg(long = "any-status")]
    pub any_status: bool,
    #[arg(long = "tag")]
    tag_filters: Vec<IdentifierToken>,
    #[arg(long = "project")]
    project_filters: Vec<IdentifierToken>,
    #[arg(long = "area")]
    area_filters: Vec<IdentifierToken>,
    #[arg(long)]
    pub inbox: bool,
    #[arg(long)]
    pub today: bool,
    #[arg(long)]
    pub someday: bool,
    #[arg(long)]
    pub evening: bool,
    #[arg(long = "has-deadline")]
    pub has_deadline: bool,
    #[arg(long = "no-deadline")]
    pub no_deadline: bool,
    #[arg(long)]
    pub recurring: bool,
    #[arg(long)]
    pub deadline: Vec<String>,
    #[arg(long)]
    pub scheduled: Vec<String>,
    #[arg(long)]
    pub created: Vec<String>,
    #[arg(long = "completed-on")]
    pub completed_on: Vec<String>,
}

impl Command for FindArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
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

        if matched.is_empty() {
            writeln!(
                out,
                "{}",
                colored("No matching tasks.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        let ids = matched
            .iter()
            .map(|(task, _)| task.uuid.clone())
            .collect::<Vec<_>>();
        let id_prefix_len = store.unique_prefix_length(&ids);
        let count = matched.len();
        let label = if count == 1 { "task" } else { "tasks" };
        writeln!(
            out,
            "{}",
            colored(
                &format!("{} Find  ({} {})", ICONS.tag, count, label),
                &[BOLD, CYAN],
                cli.no_color,
            )
        )?;
        writeln!(out)?;

        for (task, result) in matched {
            let force_detailed = self.detailed.detailed || result.checklist_only;
            writeln!(
                out,
                "{}",
                fmt_result(&task, &store, id_prefix_len, force_detailed, &today, cli.no_color,)
            )?;
        }

        Ok(())
    }
}

fn parse_date_value(value: &str, flag: &str, today: &DateTime<Utc>) -> Result<DateTime<Utc>, String> {
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

fn parse_date_expr(raw: &str, flag: &str, today: &DateTime<Utc>) -> Result<(&'static str, DateTime<Utc>), String> {
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
    resolved_tag_uuids: &[String],
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
        let Some(project) = store.get_task(&project_uuid) else {
            return MatchResult::no();
        };

        let project_title = project.title.to_ascii_lowercase();
        let matched = args
            .project_filters
            .iter()
            .any(|f| matches_project_filter(f, &project_uuid, &project_title));
        if !matched {
            return MatchResult::no();
        }
    }

    if !args.area_filters.is_empty() {
        let Some(area_uuid) = store.effective_area_uuid(task) else {
            return MatchResult::no();
        };
        let Some(area) = store.get_area(&area_uuid) else {
            return MatchResult::no();
        };

        let area_title = area.title.to_ascii_lowercase();
        let matched = args
            .area_filters
            .iter()
            .any(|f| matches_area_filter(f, &area_uuid, &area_title));
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

fn fmt_result(
    task: &Task,
    store: &ThingsStore,
    id_prefix_len: usize,
    detailed: bool,
    today: &DateTime<Utc>,
    no_color: bool,
) -> String {
    if task.is_project() {
        return fmt_project_with_note(
            task,
            store,
            "  ",
            Some(id_prefix_len),
            true,
            false,
            detailed,
            today,
            no_color,
        );
    }

    let line = fmt_task_line(
        task,
        store,
        true,
        true,
        false,
        Some(id_prefix_len),
        today,
        no_color,
    );
    fmt_task_with_note(line, task, "  ", Some(id_prefix_len), detailed, no_color)
}
