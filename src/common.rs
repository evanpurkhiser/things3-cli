use crate::store::{ChecklistItem, Tag, Task, ThingsStore};
use crate::wire::TaskStart;
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use crc32fast::Hasher;
use serde_json::{Value, json};
use std::collections::HashSet;

/// Return today as a UTC midnight `DateTime<Utc>`.
///
/// If the `THINGS3_TODAY` environment variable is set to a Unix timestamp
/// (integer seconds), that value is used instead of the system clock.
/// This allows deterministic tests without any real-time dependency.
pub fn today_utc() -> DateTime<Utc> {
    if let Ok(val) = std::env::var("THINGS3_TODAY")
        && let Ok(ts) = val.trim().parse::<i64>()
    {
        return Utc
            .timestamp_opt(ts, 0)
            .single()
            .unwrap_or_else(Utc::now)
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|d| Utc.from_utc_datetime(&d))
            .unwrap_or_else(Utc::now);
    }
    let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    Utc.from_utc_datetime(&today)
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
    dt.map(|d| d.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

pub fn fmt_deadline(deadline: Option<DateTime<Utc>>, no_color: bool) -> String {
    let Some(deadline) = deadline else {
        return String::new();
    };
    let now = today_utc();
    let color = if deadline < now { RED } else { YELLOW };
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
    id_prefix_len: Option<usize>,
    no_color: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let box_text = colored(task_box(task), &[DIM], no_color);
    parts.push(box_text);

    if show_today_markers {
        if task.evening {
            parts.push(colored(ICONS.evening, &[BLUE], no_color));
        } else if task.is_today() {
            parts.push(colored(ICONS.today, &[YELLOW], no_color));
        }
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
        parts.push(fmt_deadline(task.deadline, no_color));
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
    id_prefix_len: Option<usize>,
    no_color: bool,
) -> String {
    let title = if project.title.is_empty() {
        colored("(untitled)", &[DIM], no_color)
    } else {
        project.title.clone()
    };
    let dl = fmt_deadline(project.deadline, no_color);

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
        } else if project.is_today() {
            status_marker = format!(" {}", colored(ICONS.today, &[YELLOW], no_color));
        }
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

pub fn fmt_project_with_note(
    project: &Task,
    store: &ThingsStore,
    indent: &str,
    id_prefix_len: Option<usize>,
    show_indicators: bool,
    detailed: bool,
    no_color: bool,
) -> String {
    let line = fmt_project_line(project, store, show_indicators, id_prefix_len, no_color);
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

pub fn parse_day(day: Option<&str>, label: &str) -> Result<Option<DateTime<Local>>, String> {
    let Some(day) = day else {
        return Ok(None);
    };
    let parsed = NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map_err(|_| format!("Invalid {label} date: {day} (expected YYYY-MM-DD)"))?;
    let local_dt = parsed
        .and_hms_opt(0, 0, 0)
        .and_then(|d| Local.from_local_datetime(&d).single())
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

pub fn is_today_from_props(task_props: &serde_json::Map<String, Value>) -> bool {
    let st = task_props.get("st").and_then(Value::as_i64).unwrap_or(0);
    if st != i32::from(TaskStart::Anytime) as i64 {
        return false;
    }
    let sr = task_props.get("sr").and_then(Value::as_i64);
    let Some(sr) = sr else {
        return false;
    };

    let today_ts_local = today_utc().timestamp();
    sr <= today_ts_local
}
