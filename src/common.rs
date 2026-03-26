use crate::store::{ChecklistItem, Tag, Task, ThingsStore};
use crate::wire::TaskStart;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use crc32fast::Hasher;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Return today as a UTC midnight `DateTime<Utc>`.
pub fn today_utc() -> DateTime<Utc> {
    let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    Utc.from_utc_datetime(&today)
}

/// Return current wall-clock unix timestamp in seconds (fractional).
pub fn now_ts_f64() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";
pub const GREEN: &str = "\x1b[32m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const RED: &str = "\x1b[31m";

#[derive(Debug, Clone, Copy)]
pub struct Icons {
    pub task_open: &'static str,
    pub task_done: &'static str,
    pub task_someday: &'static str,
    pub task_canceled: &'static str,
    pub evening: &'static str,
    pub today: &'static str,
    pub today_staged: &'static str,
    pub project: &'static str,
    pub area: &'static str,
    pub tag: &'static str,
    pub inbox: &'static str,
    pub anytime: &'static str,
    pub upcoming: &'static str,
    pub progress_empty: &'static str,
    pub progress_quarter: &'static str,
    pub progress_half: &'static str,
    pub progress_three_quarter: &'static str,
    pub progress_full: &'static str,
    pub deadline: &'static str,
    pub done: &'static str,
    pub incomplete: &'static str,
    pub canceled: &'static str,
    pub deleted: &'static str,
    pub checklist_open: &'static str,
    pub checklist_done: &'static str,
    pub checklist_canceled: &'static str,
    pub separator: &'static str,
    pub divider: &'static str,
}

pub const ICONS: Icons = Icons {
    task_open: "▢",
    task_done: "◼",
    task_someday: "⬚",
    task_canceled: "☒",
    evening: "☽",
    today: "⭑",
    today_staged: "●",
    project: "●",
    area: "◆",
    tag: "⌗",
    inbox: "⬓",
    anytime: "◌",
    upcoming: "▷",
    progress_empty: "◯",
    progress_quarter: "◔",
    progress_half: "◑",
    progress_three_quarter: "◕",
    progress_full: "◉",
    deadline: "⚑",
    done: "✓",
    incomplete: "↺",
    canceled: "☒",
    deleted: "×",
    checklist_open: "○",
    checklist_done: "●",
    checklist_canceled: "×",
    separator: "·",
    divider: "─",
};

pub fn colored(text: &str, codes: &[&str], no_color: bool) -> String {
    if no_color {
        return text.to_string();
    }
    let mut out = String::new();
    for code in codes {
        out.push_str(code);
    }
    out.push_str(text);
    out.push_str(RESET);
    out
}

pub fn fmt_date(dt: Option<DateTime<Utc>>) -> String {
    dt.map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

pub fn fmt_date_local(dt: Option<DateTime<Utc>>) -> String {
    let fixed_local = fixed_local_offset();
    dt.map(|d| {
        d.with_timezone(&fixed_local)
            .format("%Y-%m-%d")
            .to_string()
    })
        .unwrap_or_default()
}

fn fixed_local_offset() -> FixedOffset {
    let seconds = Local::now().offset().local_minus_utc();
    FixedOffset::east_opt(seconds).unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset"))
}

pub fn fmt_deadline(deadline: Option<DateTime<Utc>>, today: &DateTime<Utc>, no_color: bool) -> String {
    let Some(deadline) = deadline else {
        return String::new();
    };
    let color = if deadline < *today { RED } else { YELLOW };
    format!(
        " {} due by {}",
        ICONS.deadline,
        colored(&fmt_date(Some(deadline)), &[color], no_color)
    )
}

fn task_box(task: &Task) -> &'static str {
    if task.is_completed() {
        ICONS.task_done
    } else if task.is_canceled() {
        ICONS.task_canceled
    } else if task.in_someday() {
        ICONS.task_someday
    } else {
        ICONS.task_open
    }
}

pub fn id_prefix(uuid: &str, size: usize, no_color: bool) -> String {
    let mut short = uuid.chars().take(size).collect::<String>();
    while short.len() < size {
        short.push(' ');
    }
    colored(&short, &[DIM], no_color)
}

pub fn fmt_task_line(
    task: &Task,
    store: &ThingsStore,
    show_project: bool,
    show_today_markers: bool,
    show_staged_today_marker: bool,
    id_prefix_len: Option<usize>,
    today: &DateTime<Utc>,
    no_color: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let box_text = colored(task_box(task), &[DIM], no_color);
    parts.push(box_text);

    if show_today_markers {
        if task.evening {
            parts.push(colored(ICONS.evening, &[BLUE], no_color));
        } else if task.is_today(today) {
            parts.push(colored(ICONS.today, &[YELLOW], no_color));
        }
    } else if show_staged_today_marker && task.is_staged_for_today(today) {
        parts.push(colored(ICONS.today_staged, &[YELLOW], no_color));
    }

    let title = if task.title.is_empty() {
        colored("(untitled)", &[DIM], no_color)
    } else {
        task.title.clone()
    };
    parts.push(title);

    if !task.tags.is_empty() {
        let tag_names: Vec<String> = task
            .tags
            .iter()
            .map(|t| store.resolve_tag_title(t))
            .collect();
        parts.push(colored(
            &format!(" [{}]", tag_names.join(", ")),
            &[DIM],
            no_color,
        ));
    }

    if show_project
        && let Some(effective_project) = store.effective_project_uuid(task)
    {
        let title = store.resolve_project_title(&effective_project);
        parts.push(colored(
            &format!(" {} {}", ICONS.separator, title),
            &[DIM],
            no_color,
        ));
    }

    if task.deadline.is_some() {
        parts.push(fmt_deadline(task.deadline, today, no_color));
    }

    let line = parts.join(" ");
    if let Some(len) = id_prefix_len
        && len > 0
    {
        return format!("{} {}", id_prefix(&task.uuid, len, no_color), line);
    }
    line
}

pub fn fmt_project_line(
    project: &Task,
    store: &ThingsStore,
    show_indicators: bool,
    show_staged_today_marker: bool,
    id_prefix_len: Option<usize>,
    today: &DateTime<Utc>,
    no_color: bool,
) -> String {
    let title = if project.title.is_empty() {
        colored("(untitled)", &[DIM], no_color)
    } else {
        project.title.clone()
    };
    let dl = fmt_deadline(project.deadline, today, no_color);

    let marker = if project.in_someday() {
        ICONS.anytime
    } else {
        let progress = store.project_progress(&project.uuid);
        let total = progress.total;
        let done = progress.done;
        if total == 0 || done == 0 {
            ICONS.progress_empty
        } else if done == total {
            ICONS.progress_full
        } else {
            let ratio = done as f32 / total as f32;
            if ratio < (1.0 / 3.0) {
                ICONS.progress_quarter
            } else if ratio < (2.0 / 3.0) {
                ICONS.progress_half
            } else {
                ICONS.progress_three_quarter
            }
        }
    };

    let mut status_marker = String::new();
    if show_indicators {
        if project.evening {
            status_marker = format!(" {}", colored(ICONS.evening, &[BLUE], no_color));
        } else if project.is_today(today) {
            status_marker = format!(" {}", colored(ICONS.today, &[YELLOW], no_color));
        }
    } else if show_staged_today_marker && project.is_staged_for_today(today) {
        status_marker = format!(" {}", colored(ICONS.today_staged, &[YELLOW], no_color));
    }

    let id_part = if let Some(len) = id_prefix_len {
        if len > 0 {
            format!("{} ", id_prefix(&project.uuid, len, no_color))
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "{}{}{} {}{}",
        id_part,
        colored(marker, &[DIM], no_color),
        status_marker,
        title,
        dl
    )
}

fn note_indent(id_prefix_len: Option<usize>) -> String {
    let width = id_prefix_len
        .unwrap_or(0)
        .saturating_add(if id_prefix_len.unwrap_or(0) > 0 { 1 } else { 0 });
    " ".repeat(width)
}

fn checklist_prefix_len(items: &[ChecklistItem]) -> usize {
    if items.is_empty() {
        return 0;
    }
    for length in 1..=22 {
        let mut set = std::collections::HashSet::new();
        let unique = items
            .iter()
            .map(|item| item.uuid.chars().take(length).collect::<String>())
            .all(|id| set.insert(id));
        if unique {
            return length;
        }
    }
    4
}

fn checklist_icon(item: &ChecklistItem, no_color: bool) -> String {
    if item.is_completed() {
        colored(ICONS.checklist_done, &[DIM], no_color)
    } else if item.is_canceled() {
        colored(ICONS.checklist_canceled, &[DIM], no_color)
    } else {
        colored(ICONS.checklist_open, &[DIM], no_color)
    }
}

pub fn fmt_task_with_note(
    line: String,
    task: &Task,
    indent: &str,
    id_prefix_len: Option<usize>,
    detailed: bool,
    no_color: bool,
) -> String {
    let mut out = vec![format!("{}{}", indent, line)];
    if !detailed {
        return out.join("\n");
    }

    let note_pad = format!("{}{}", indent, note_indent(id_prefix_len));
    let has_checklist = !task.checklist_items.is_empty();
    let pipe = colored("│", &[DIM], no_color);
    let note_lines: Vec<String> = task
        .notes
        .as_ref()
        .map(|n| n.lines().map(ToString::to_string).collect())
        .unwrap_or_default();

    if has_checklist {
        let items = &task.checklist_items;
        let cl_prefix_len = checklist_prefix_len(items);
        let col = id_prefix_len.unwrap_or(0);
        if !note_lines.is_empty() {
            for note_line in &note_lines {
                out.push(format!(
                    "{}{} {} {}",
                    indent,
                    " ".repeat(col),
                    pipe,
                    colored(note_line, &[DIM], no_color)
                ));
            }
            out.push(format!("{}{} {}", indent, " ".repeat(col), pipe));
        }

        for (i, item) in items.iter().enumerate() {
            let connector = colored(
                if i == items.len() - 1 {
                    "└╴"
                } else {
                    "├╴"
                },
                &[DIM],
                no_color,
            );
            let cl_id_raw = item.uuid.chars().take(cl_prefix_len).collect::<String>();
            let cl_id = colored(
                &format!("{:>width$}", cl_id_raw, width = col),
                &[DIM],
                no_color,
            );
            out.push(format!(
                "{}{} {}{} {}",
                indent,
                cl_id,
                connector,
                checklist_icon(item, no_color),
                item.title
            ));
        }
    } else if !note_lines.is_empty() {
        for note_line in note_lines.iter().take(note_lines.len().saturating_sub(1)) {
            out.push(format!(
                "{}{} {}",
                note_pad,
                pipe,
                colored(note_line, &[DIM], no_color)
            ));
        }
        if let Some(last) = note_lines.last() {
            out.push(format!(
                "{}{} {}",
                note_pad,
                colored("└", &[DIM], no_color),
                colored(last, &[DIM], no_color)
            ));
        }
    }

    out.join("\n")
}

#[allow(clippy::too_many_arguments)]
pub fn fmt_project_with_note(
    project: &Task,
    store: &ThingsStore,
    indent: &str,
    id_prefix_len: Option<usize>,
    show_indicators: bool,
    show_staged_today_marker: bool,
    detailed: bool,
    today: &DateTime<Utc>,
    no_color: bool,
) -> String {
    let line = fmt_project_line(
        project,
        store,
        show_indicators,
        show_staged_today_marker,
        id_prefix_len,
        today,
        no_color,
    );
    let mut out = vec![format!("{}{}", indent, line)];

    if detailed
        && let Some(notes) = &project.notes
    {
        let width =
            id_prefix_len.unwrap_or(0) + if id_prefix_len.unwrap_or(0) > 0 { 1 } else { 0 };
        let note_pad = format!("{}{}", indent, " ".repeat(width));
        let lines: Vec<&str> = notes.lines().collect();
        for note in lines.iter().take(lines.len().saturating_sub(1)) {
            out.push(format!(
                "{}{} {}",
                note_pad,
                colored("│", &[DIM], no_color),
                colored(note, &[DIM], no_color)
            ));
        }
        if let Some(last) = lines.last() {
            out.push(format!(
                "{}{} {}",
                note_pad,
                colored("└", &[DIM], no_color),
                colored(last, &[DIM], no_color)
            ));
        }
    }

    out.join("\n")
}

#[derive(Default)]
struct AreaTaskGroup<'a> {
    tasks: Vec<&'a Task>,
    projects: Vec<(String, Vec<&'a Task>)>,
    project_pos: HashMap<String, usize>,
}

#[allow(clippy::too_many_arguments)]
pub fn fmt_tasks_grouped(
    tasks: &[Task],
    store: &ThingsStore,
    indent: &str,
    show_today_markers: bool,
    detailed: bool,
    today: &DateTime<Utc>,
    no_color: bool,
) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    const MAX_GROUP_ITEMS: usize = 3;

    let mut unscoped: Vec<&Task> = Vec::new();

    let mut project_only: Vec<(String, Vec<&Task>)> = Vec::new();
    let mut project_only_pos: HashMap<String, usize> = HashMap::new();

    let mut by_area: Vec<(String, AreaTaskGroup<'_>)> = Vec::new();
    let mut by_area_pos: HashMap<String, usize> = HashMap::new();

    for task in tasks {
        let project_uuid = store.effective_project_uuid(task);
        let area_uuid = store.effective_area_uuid(task);

        match (project_uuid, area_uuid) {
            (Some(project_uuid), Some(area_uuid)) => {
                let area_idx = if let Some(i) = by_area_pos.get(&area_uuid).copied() {
                    i
                } else {
                    let i = by_area.len();
                    by_area.push((area_uuid.clone(), AreaTaskGroup::default()));
                    by_area_pos.insert(area_uuid.clone(), i);
                    i
                };
                let area_group = &mut by_area[area_idx].1;

                let project_idx = if let Some(i) = area_group.project_pos.get(&project_uuid).copied() {
                    i
                } else {
                    let i = area_group.projects.len();
                    area_group.projects.push((project_uuid.clone(), Vec::new()));
                    area_group.project_pos.insert(project_uuid.clone(), i);
                    i
                };
                area_group.projects[project_idx].1.push(task);
            }
            (Some(project_uuid), None) => {
                let project_idx = if let Some(i) = project_only_pos.get(&project_uuid).copied() {
                    i
                } else {
                    let i = project_only.len();
                    project_only.push((project_uuid.clone(), Vec::new()));
                    project_only_pos.insert(project_uuid.clone(), i);
                    i
                };
                project_only[project_idx].1.push(task);
            }
            (None, Some(area_uuid)) => {
                let area_idx = if let Some(i) = by_area_pos.get(&area_uuid).copied() {
                    i
                } else {
                    let i = by_area.len();
                    by_area.push((area_uuid.clone(), AreaTaskGroup::default()));
                    by_area_pos.insert(area_uuid.clone(), i);
                    i
                };
                by_area[area_idx].1.tasks.push(task);
            }
            (None, None) => {
                unscoped.push(task);
            }
        }
    }

    let mut ids: Vec<String> = tasks.iter().map(|t| t.uuid.clone()).collect();
    for (project_uuid, _) in &project_only {
        ids.push(project_uuid.clone());
    }
    for (area_uuid, area_group) in &by_area {
        if !area_uuid.is_empty() {
            ids.push(area_uuid.clone());
        }
        for (project_uuid, _) in &area_group.projects {
            ids.push(project_uuid.clone());
        }
    }
    let id_prefix_len = store.unique_prefix_length(&ids);

    let mut sections: Vec<String> = Vec::new();

    if !unscoped.is_empty() {
        let mut lines: Vec<String> = Vec::new();
        for task in unscoped {
            let line = fmt_task_line(
                task,
                store,
                false,
                show_today_markers,
                false,
                Some(id_prefix_len),
                today,
                no_color,
            );
            lines.push(fmt_task_with_note(
                line,
                task,
                indent,
                Some(id_prefix_len),
                detailed,
                no_color,
            ));
        }
        sections.push(lines.join("\n"));
    }

    let fmt_limited_tasks = |group_tasks: &[&Task], task_indent: &str| -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for task in group_tasks.iter().take(MAX_GROUP_ITEMS) {
            let line = fmt_task_line(
                task,
                store,
                false,
                show_today_markers,
                false,
                Some(id_prefix_len),
                today,
                no_color,
            );
            lines.push(fmt_task_with_note(
                line,
                task,
                task_indent,
                Some(id_prefix_len),
                detailed,
                no_color,
            ));
        }
        let hidden = group_tasks.len().saturating_sub(MAX_GROUP_ITEMS);
        if hidden > 0 {
            lines.push(colored(
                &format!("{task_indent}Hiding {hidden} more"),
                &[DIM],
                no_color,
            ));
        }
        lines
    };

    for (project_uuid, project_tasks) in &project_only {
        let title = store.resolve_project_title(project_uuid);
        let mut lines = vec![format!(
            "{}{} {}",
            indent,
            id_prefix(project_uuid, id_prefix_len, no_color),
            colored(&format!("{} {}", ICONS.project, title), &[BOLD], no_color)
        )];
        lines.extend(fmt_limited_tasks(project_tasks, &format!("{}  ", indent)));
        sections.push(lines.join("\n"));
    }

    for (area_uuid, area_group) in &by_area {
        let area_title = store.resolve_area_title(area_uuid);
        let mut lines = vec![format!(
            "{}{} {}",
            indent,
            id_prefix(area_uuid, id_prefix_len, no_color),
            colored(&format!("{} {}", ICONS.area, area_title), &[BOLD], no_color)
        )];

        lines.extend(fmt_limited_tasks(&area_group.tasks, &format!("{}  ", indent)));

        for (project_uuid, project_tasks) in &area_group.projects {
            let project_title = store.resolve_project_title(project_uuid);
            lines.push(format!(
                "{}  {} {}",
                indent,
                id_prefix(project_uuid, id_prefix_len, no_color),
                colored(&format!("{} {}", ICONS.project, project_title), &[BOLD], no_color)
            ));
            lines.extend(fmt_limited_tasks(project_tasks, &format!("{}    ", indent)));
        }

        sections.push(lines.join("\n"));
    }

    sections.join("\n\n")
}

pub fn parse_day(day: Option<&str>, label: &str) -> Result<Option<DateTime<Local>>, String> {
    let Some(day) = day else {
        return Ok(None);
    };
    let parsed = NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map_err(|_| format!("Invalid {label} date: {day} (expected YYYY-MM-DD)"))?;
    let fixed_local = fixed_local_offset();
    let local_dt = parsed
        .and_hms_opt(0, 0, 0)
        .and_then(|d| fixed_local.from_local_datetime(&d).single())
        .map(|d| d.with_timezone(&Local))
        .ok_or_else(|| format!("Invalid {label} date: {day} (expected YYYY-MM-DD)"))?;
    Ok(Some(local_dt))
}

pub fn day_to_timestamp(day: DateTime<Local>) -> i64 {
    day.with_timezone(&Utc).timestamp()
}

pub fn task6_note(value: &str) -> Value {
    let mut hasher = Hasher::new();
    hasher.update(value.as_bytes());
    let checksum = hasher.finalize();
    json!({"_t": "tx", "t": 1, "ch": checksum, "v": value})
}

pub fn resolve_single_tag(store: &ThingsStore, identifier: &str) -> (Option<Tag>, String) {
    let identifier = identifier.trim();
    let all_tags = store.tags();

    let exact = all_tags
        .iter()
        .filter(|t| t.title.eq_ignore_ascii_case(identifier))
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return (exact.first().cloned(), String::new());
    }
    if exact.len() > 1 {
        return (None, format!("Ambiguous tag title: {identifier}"));
    }

    let prefix = all_tags
        .iter()
        .filter(|t| t.uuid.starts_with(identifier))
        .cloned()
        .collect::<Vec<_>>();
    if prefix.len() == 1 {
        return (prefix.first().cloned(), String::new());
    }
    if prefix.len() > 1 {
        return (None, format!("Ambiguous tag UUID prefix: {identifier}"));
    }

    (None, format!("Tag not found: {identifier}"))
}

pub fn resolve_tag_ids(store: &ThingsStore, raw_tags: &str) -> (Vec<String>, String) {
    let tokens = raw_tags
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return (Vec::new(), String::new());
    }

    let all_tags = store.tags();
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();

    for token in tokens {
        let exact = all_tags
            .iter()
            .filter(|tag| tag.title.eq_ignore_ascii_case(token))
            .cloned()
            .collect::<Vec<_>>();

        if exact.len() == 1 {
            let tag_uuid = exact[0].uuid.clone();
            if seen.insert(tag_uuid.clone()) {
                resolved.push(tag_uuid);
            }
            continue;
        }
        if exact.len() > 1 {
            return (Vec::new(), format!("Ambiguous tag title: {token}"));
        }

        let prefix = all_tags
            .iter()
            .filter(|tag| tag.uuid.starts_with(token))
            .cloned()
            .collect::<Vec<_>>();

        if prefix.len() == 1 {
            let tag_uuid = prefix[0].uuid.clone();
            if seen.insert(tag_uuid.clone()) {
                resolved.push(tag_uuid);
            }
            continue;
        }
        if prefix.len() > 1 {
            return (Vec::new(), format!("Ambiguous tag UUID prefix: {token}"));
        }

        return (Vec::new(), format!("Tag not found: {token}"));
    }

    (resolved, String::new())
}

pub fn is_today_from_props(
    task_props: &serde_json::Map<String, Value>,
    today_ts: i64,
) -> bool {
    let st = task_props.get("st").and_then(Value::as_i64).unwrap_or(0);
    if st != i32::from(TaskStart::Anytime) as i64 {
        return false;
    }
    let sr = task_props.get("sr").and_then(Value::as_i64);
    let Some(sr) = sr else {
        return false;
    };

    let today_ts_local = today_ts;
    sr <= today_ts_local
}
