"""Shared formatting helpers, color constants, icons, and utilities."""

import argparse
import sys
import zlib
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Callable, Optional

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore, Task, Tag, ChecklistItem

RECURRENCE_FIXED_SCHEDULE = 0
RECURRENCE_AFTER_COMPLETION = 1
LOCAL_TZ = datetime.now().astimezone().tzinfo or timezone.utc

# ---------------------------------------------------------------------------
# Shared parent parsers
# ---------------------------------------------------------------------------

# Parser that adds --detailed flag; used as a parent by view commands.
detailed_parent = argparse.ArgumentParser(add_help=False)
detailed_parent.add_argument(
    "--detailed",
    action="store_true",
    help="Show notes beneath each task",
)

# Parser that adds --add-tags / --remove-tags; used as a parent by edit commands.
tag_edit_parent = argparse.ArgumentParser(add_help=False)
tag_edit_parent.add_argument(
    "--add-tags",
    dest="add_tags",
    help="Comma-separated tags to add (titles or UUID prefixes)",
)
tag_edit_parent.add_argument(
    "--remove-tags",
    dest="remove_tags",
    help="Comma-separated tags to remove (titles or UUID prefixes)",
)

# ---------------------------------------------------------------------------
# Color constants
# ---------------------------------------------------------------------------

RESET = "\033[0m"
BOLD = "\033[1m"
DIM = "\033[2m"
CYAN = "\033[36m"
YELLOW = "\033[33m"
GREEN = "\033[32m"
BLUE = "\033[34m"
MAGENTA = "\033[35m"
RED = "\033[31m"


@dataclass(frozen=True)
class _Icons:
    # Task checkboxes
    task_open: str = "▢"
    task_done: str = "◼"
    task_someday: str = "⬚"
    task_canceled: str = "☒"

    # Time-of-day markers
    evening: str = "☽"
    today: str = "⭑"
    today_staged: str = "●"

    # Entity icons
    project: str = "●"
    area: str = "◆"
    tag: str = "⌗"
    inbox: str = "⬓"
    anytime: str = "◌"
    upcoming: str = "▷"

    # Project progress pies
    progress_empty: str = "◯"
    progress_quarter: str = "◔"
    progress_half: str = "◑"
    progress_three_quarter: str = "◕"
    progress_full: str = "◉"

    # Status indicators
    deadline: str = "⚑"
    done: str = "✓"
    incomplete: str = "↺"
    canceled: str = "☒"
    deleted: str = "×"

    # Checklist items
    checklist_open: str = "○"
    checklist_done: str = "●"
    checklist_canceled: str = "×"

    # Misc
    separator: str = "·"
    divider: str = "─"


ICONS = _Icons()


# ---------------------------------------------------------------------------
# Type aliases
# ---------------------------------------------------------------------------

CommandHandler = Callable[
    [ThingsStore, argparse.Namespace, ThingsCloudClient], Optional[int]
]
StoreCommand = Callable[[ThingsStore, argparse.Namespace], None]


def _adapt_store_command(command: StoreCommand) -> CommandHandler:
    def handler(
        store: ThingsStore, args: argparse.Namespace, _client: ThingsCloudClient
    ) -> Optional[int]:
        command(store, args)
        return None

    return handler


# ---------------------------------------------------------------------------
# Formatting helpers
# ---------------------------------------------------------------------------


def colored(text: str, *codes: str) -> str:
    return "".join(codes) + text + RESET


def fmt_date(dt: Optional[datetime]) -> str:
    """Format a datetime as YYYY-MM-DD.

    Things stores dates as UTC midnight, so we use UTC for date display
    to avoid off-by-one day errors from timezone conversion.
    """
    if dt is None:
        return ""
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%d")


def fmt_date_local(dt: Optional[datetime]) -> str:
    """Format a datetime as YYYY-MM-DD in local timezone."""
    if dt is None:
        return ""
    return dt.astimezone(LOCAL_TZ).strftime("%Y-%m-%d")


def fmt_deadline(deadline: Optional[datetime]) -> str:
    """Format a deadline as a colored '⚑ due by YYYY-MM-DD' string, red if overdue."""
    if not deadline:
        return ""
    now = datetime.now(tz=timezone.utc)
    color = RED if deadline < now else YELLOW
    return colored(f" {ICONS.deadline} due by {fmt_date(deadline)}", color)


def _task6_note(value: str) -> dict:
    payload = value or ""
    checksum = zlib.crc32(payload.encode("utf-8")) & 0xFFFFFFFF
    return {"_t": "tx", "t": 1, "ch": checksum, "v": payload}


def _task_box(task: Task) -> str:
    if task.is_completed:
        return ICONS.task_done
    if task.is_canceled:
        return ICONS.task_canceled
    if task.in_someday:
        return ICONS.task_someday
    return ICONS.task_open


def _id_prefix(uuid: str, size: int) -> str:
    return colored(uuid[:size].ljust(size), DIM)


@dataclass
class AreaTaskGroup:
    tasks: list[Task] = field(default_factory=list)
    projects: dict[str, list[Task]] = field(default_factory=dict)


def fmt_task_line(
    task: Task,
    store: ThingsStore,
    show_project: bool = False,
    show_today_markers: bool = False,
    show_staged_today_marker: bool = False,
    id_prefix_len: Optional[int] = None,
) -> str:
    """Format a single task for terminal output."""
    parts = []

    # Checkbox
    box = _task_box(task)
    parts.append(colored(box, DIM))

    if show_today_markers:
        if task.evening:
            parts.append(colored(ICONS.evening, BLUE))
        elif task.is_today:
            parts.append(colored(ICONS.today, YELLOW))
    elif show_staged_today_marker and task.is_staged_for_today:
        parts.append(colored(ICONS.today_staged, YELLOW))

    # Title
    title = task.title or colored("(untitled)", DIM)
    parts.append(title)

    # Tags
    if task.tags:
        tag_names = [store.resolve_tag_title(t) for t in task.tags]
        parts.append(colored(" [" + ", ".join(tag_names) + "]", DIM))

    # Project
    effective_project = store.effective_project_uuid(task)
    if show_project and effective_project:
        proj_title = store.resolve_project_title(effective_project)
        parts.append(colored(f" {ICONS.separator} {proj_title}", DIM))

    # Deadline
    if task.deadline:
        parts.append(fmt_deadline(task.deadline))

    line = " ".join(parts) if parts else title
    if id_prefix_len and id_prefix_len > 0:
        return f"{_id_prefix(task.uuid, id_prefix_len)} {line}"
    return line


def fmt_project_line(
    project: Task,
    store: ThingsStore,
    show_indicators: bool = True,
    show_staged_today_marker: bool = False,
    id_prefix_len: Optional[int] = None,
) -> str:
    """Format a single project for terminal output."""
    title = project.title or colored("(untitled)", DIM)
    dl = fmt_deadline(project.deadline)

    if project.in_someday:
        marker = ICONS.anytime
    else:
        progress = store.project_progress(project.uuid)
        total = progress.total
        done = progress.done

        if total == 0 or done == 0:
            marker = ICONS.progress_empty
        elif done == total:
            marker = ICONS.progress_full
        else:
            ratio = done / total
            if ratio < 1 / 3:
                marker = ICONS.progress_quarter
            elif ratio < 2 / 3:
                marker = ICONS.progress_half
            else:
                marker = ICONS.progress_three_quarter

    status_marker = ""
    if show_indicators:
        if project.evening:
            status_marker = f" {colored(ICONS.evening, BLUE)}"
        elif project.is_today:
            status_marker = f" {colored(ICONS.today, YELLOW)}"
    elif show_staged_today_marker and project.is_staged_for_today:
        status_marker = f" {colored(ICONS.today_staged, YELLOW)}"

    id_part = f"{_id_prefix(project.uuid, id_prefix_len)} " if id_prefix_len else ""
    return f"{id_part}{colored(marker, DIM)}{status_marker} {title}{dl}"


def _note_indent(
    id_prefix_len: Optional[int],
) -> str:
    """Return the indent string to align tree/note lines under the task checkbox."""
    width = id_prefix_len + 1 if id_prefix_len and id_prefix_len > 0 else 0
    return " " * width


def _checklist_prefix_len(items: list[ChecklistItem]) -> int:
    """Minimum prefix length to uniquely identify each item within this task's checklist."""
    if not items:
        return 0
    for length in range(1, 23):
        if len({item.uuid[:length] for item in items}) == len(items):
            return length
    return 4  # fallback, should never be needed


def _checklist_icon(item: ChecklistItem) -> str:
    if item.is_completed:
        return colored(ICONS.checklist_done, DIM)
    if item.is_canceled:
        return colored(ICONS.checklist_canceled, DIM)
    return colored(ICONS.checklist_open, DIM)


def fmt_task_with_note(
    line: str,
    task: Task,
    indent: str,
    show_today_markers: bool = False,
    id_prefix_len: Optional[int] = None,
    detailed: bool = False,
) -> str:
    """Return a formatted task line, and optionally its note beneath it."""
    out = [indent + line]
    if not detailed:
        return "\n".join(out)

    note_pad = indent + _note_indent(id_prefix_len)
    has_checklist = bool(task.checklist_items)

    pipe = colored("│", DIM)
    note_lines = task.notes.splitlines() if task.notes else []

    if has_checklist:
        items = task.checklist_items
        cl_prefix_len = _checklist_prefix_len(items)
        col = id_prefix_len or 0
        if note_lines:
            for note_line in note_lines:
                out.append(f"{indent}{' ' * col} {pipe} {colored(note_line, DIM)}")
            out.append(f"{indent}{' ' * col} {pipe}")
        for i, item in enumerate(items):
            connector = colored("└╴" if i == len(items) - 1 else "├╴", DIM)
            cl_id = colored(item.uuid[:cl_prefix_len].rjust(col), DIM)
            out.append(
                f"{indent}{cl_id} {connector}{_checklist_icon(item)} {item.title}"
            )
    elif note_lines:
        for note_line in note_lines[:-1]:
            out.append(f"{note_pad}{pipe} {colored(note_line, DIM)}")
        out.append(f"{note_pad}{colored('└', DIM)} {colored(note_lines[-1], DIM)}")

    return "\n".join(out)


def fmt_project_with_note(
    project: Task,
    store: ThingsStore,
    indent: str,
    id_prefix_len: Optional[int] = None,
    show_indicators: bool = True,
    detailed: bool = False,
) -> str:
    """Return a formatted project line, and optionally its note beneath it."""
    line = fmt_project_line(
        project, store, show_indicators=show_indicators, id_prefix_len=id_prefix_len
    )
    out = [indent + line]
    if detailed and project.notes:
        # align under the progress marker (id_prefix + space + marker)
        width = id_prefix_len + 1 if id_prefix_len else 0
        note_pad = indent + " " * width
        note_lines = project.notes.splitlines()
        for note_line in note_lines[:-1]:
            out.append(f"{note_pad}{colored('│', DIM)} {colored(note_line, DIM)}")
        out.append(f"{note_pad}{colored('└', DIM)} {colored(note_lines[-1], DIM)}")
    return "\n".join(out)


def fmt_section(
    title: str, tasks: list[Task], store: ThingsStore, show_project: bool = False
) -> str:
    if not tasks:
        return ""
    out = [
        colored(f"\n{title}", BOLD + CYAN),
        colored(ICONS.divider * 40, DIM),
    ]
    for task in tasks:
        out.append("  " + fmt_task_line(task, store, show_project=show_project))
    return "\n".join(out)


def fmt_tasks_grouped(
    tasks: list[Task],
    store: ThingsStore,
    indent: str = "  ",
    show_today_markers: bool = False,
    id_prefix_len: Optional[int] = None,
    detailed: bool = False,
) -> str:
    """Return tasks grouped by area and project, preserving first-seen order."""
    max_group_items = 3

    def fmt_limited_tasks(group_tasks: list[Task], task_indent: str) -> list[str]:
        out: list[str] = []
        shown = group_tasks[:max_group_items]
        for task in shown:
            line = fmt_task_line(
                task,
                store,
                show_project=False,
                show_today_markers=show_today_markers,
                id_prefix_len=id_prefix_len,
            )
            out.append(
                fmt_task_with_note(
                    line,
                    task,
                    task_indent,
                    show_today_markers=show_today_markers,
                    id_prefix_len=id_prefix_len,
                    detailed=detailed,
                )
            )
        hidden = len(group_tasks) - len(shown)
        if hidden > 0:
            out.append(colored(f"{task_indent}Hiding {hidden} more", DIM))
        return out

    if not tasks:
        return ""

    unscoped: list[Task] = []
    project_only: dict[str, list[Task]] = {}
    by_area: dict[str, AreaTaskGroup] = {}

    for task in tasks:
        project_uuid = store.effective_project_uuid(task)
        area_uuid = store.effective_area_uuid(task)

        if project_uuid:
            if area_uuid:
                if area_uuid not in by_area:
                    by_area[area_uuid] = AreaTaskGroup()
                area_projects = by_area[area_uuid].projects
                if project_uuid not in area_projects:
                    area_projects[project_uuid] = []
                area_projects[project_uuid].append(task)
            else:
                if project_uuid not in project_only:
                    project_only[project_uuid] = []
                project_only[project_uuid].append(task)
        elif area_uuid:
            if area_uuid not in by_area:
                by_area[area_uuid] = AreaTaskGroup()
            by_area[area_uuid].tasks.append(task)
        else:
            unscoped.append(task)

    if id_prefix_len is None:
        ids = [task.uuid for task in tasks]
        ids.extend(project_only.keys())
        ids.extend(area for area in by_area.keys() if area)
        for area_group in by_area.values():
            ids.extend(area_group.projects.keys())
        id_prefix_len = store.unique_prefix_length(ids)

    sections: list[list[str]] = []

    if unscoped:
        group: list[str] = []
        for task in unscoped:
            line = fmt_task_line(
                task,
                store,
                show_project=False,
                show_today_markers=show_today_markers,
                id_prefix_len=id_prefix_len,
            )
            group.append(
                fmt_task_with_note(
                    line,
                    task,
                    indent,
                    show_today_markers=show_today_markers,
                    id_prefix_len=id_prefix_len,
                    detailed=detailed,
                )
            )
        sections.append(group)

    for project_uuid, project_tasks in project_only.items():
        title = store.resolve_project_title(project_uuid)
        group = [
            f"{indent}{_id_prefix(project_uuid, id_prefix_len)} {colored(f'{ICONS.project} {title}', BOLD)}"
        ]
        group.extend(fmt_limited_tasks(project_tasks, indent + "  "))
        sections.append(group)

    for area_uuid, area_group in by_area.items():
        area_title = store.resolve_area_title(area_uuid)
        group = [
            f"{indent}{_id_prefix(area_uuid, id_prefix_len)} {colored(f'{ICONS.area} {area_title}', BOLD)}"
        ]
        group.extend(fmt_limited_tasks(area_group.tasks, indent + "  "))
        for project_uuid, project_tasks in area_group.projects.items():
            project_title = store.resolve_project_title(project_uuid)
            group.append(
                f"{indent}  {_id_prefix(project_uuid, id_prefix_len)} "
                + colored(f"{ICONS.project} {project_title}", BOLD)
            )
            group.extend(fmt_limited_tasks(project_tasks, indent + "    "))
        sections.append(group)

    return "\n\n".join("\n".join(section) for section in sections)


# ---------------------------------------------------------------------------
# Date helpers
# ---------------------------------------------------------------------------


def _parse_day(day: Optional[str], label: str) -> Optional[datetime]:
    if not day:
        return None
    try:
        parsed = datetime.strptime(day, "%Y-%m-%d")
    except ValueError:
        raise ValueError(f"Invalid {label} date: {day} (expected YYYY-MM-DD)")
    return parsed.replace(tzinfo=LOCAL_TZ)


def _day_to_timestamp(day: datetime) -> int:
    return int(day.astimezone(timezone.utc).timestamp())


# ---------------------------------------------------------------------------
# Tag resolution
# ---------------------------------------------------------------------------


def fmt_resolve_error(
    err: str,
    ambiguous: list[Task],
    store: ThingsStore,
) -> None:
    """Print a resolution error and any ambiguous candidates to stderr."""
    print(err, file=sys.stderr)
    if ambiguous:
        id_prefix_len = store.unique_prefix_length([t.uuid for t in ambiguous])
        for match in ambiguous:
            if match.is_project:
                print(
                    f"  {fmt_project_line(match, store, id_prefix_len=id_prefix_len)}"
                )
            else:
                print(
                    f"  {fmt_task_line(match, store, show_project=True, id_prefix_len=id_prefix_len)}"
                )


def _resolve_checklist_items(
    task: "Task", raw_ids: str
) -> tuple[list["ChecklistItem"], str]:
    """Resolve comma-separated short ID prefixes against a task's checklist items.

    Returns (matched_items, error_message). Error is non-empty on any failure.
    """
    tokens = [t.strip() for t in raw_ids.split(",") if t.strip()]
    if not tokens:
        return [], "No checklist item IDs provided."

    items = task.checklist_items
    resolved: list[ChecklistItem] = []
    seen: set[str] = set()

    for token in tokens:
        matches = [item for item in items if item.uuid.startswith(token)]
        if not matches:
            return [], f"Checklist item not found: {token!r}"
        if len(matches) > 1:
            return [], f"Ambiguous checklist item prefix: {token!r}"
        item = matches[0]
        if item.uuid not in seen:
            seen.add(item.uuid)
            resolved.append(item)

    return resolved, ""


def _apply_tag_changes(
    current_tags: list[str],
    add_raw: Optional[str],
    remove_raw: Optional[str],
    store: ThingsStore,
) -> tuple[Optional[list[str]], list[str], str]:
    """Compute the new tag list after adding and/or removing tags.

    Returns ``(new_tag_list, labels, error)``.
    On error, new_tag_list is None and error is a non-empty message.
    If no changes are requested, returns (None, [], "").
    """
    labels: list[str] = []
    result = list(current_tags)

    if add_raw:
        add_ids, err = _resolve_tag_ids(store, add_raw)
        if err:
            return None, [], err
        for uuid in add_ids:
            if uuid not in result:
                result.append(uuid)
        labels.append("add-tags")

    if remove_raw:
        remove_ids, err = _resolve_tag_ids(store, remove_raw)
        if err:
            return None, [], err
        result = [uuid for uuid in result if uuid not in remove_ids]
        labels.append("remove-tags")

    if not labels:
        return None, [], ""

    return result, labels, ""


def _resolve_single_tag(
    store: ThingsStore, identifier: str
) -> tuple[Optional[Tag], str]:
    """Resolve a single tag by title or UUID prefix.

    Returns ``(tag, error_message)``. On success tag is set and error is empty.
    """
    identifier = identifier.strip()
    all_tags = store.tags()

    exact = [t for t in all_tags if t.title.lower() == identifier.lower()]
    if len(exact) == 1:
        return exact[0], ""
    if len(exact) > 1:
        return None, f"Ambiguous tag title: {identifier}"

    prefix = [t for t in all_tags if t.uuid.startswith(identifier)]
    if len(prefix) == 1:
        return prefix[0], ""
    if len(prefix) > 1:
        return None, f"Ambiguous tag UUID prefix: {identifier}"

    return None, f"Tag not found: {identifier}"


def _resolve_tag_ids(store: ThingsStore, raw_tags: str) -> tuple[list[str], str]:
    tokens = [part.strip() for part in raw_tags.split(",") if part.strip()]
    if not tokens:
        return [], ""

    all_tags = store.tags()
    resolved: list[str] = []
    seen: set[str] = set()

    for token in tokens:
        token_l = token.lower()

        exact = [tag for tag in all_tags if tag.title.lower() == token_l]
        if len(exact) == 1:
            tag_uuid = exact[0].uuid
            if tag_uuid not in seen:
                seen.add(tag_uuid)
                resolved.append(tag_uuid)
            continue
        if len(exact) > 1:
            return [], f"Ambiguous tag title: {token}"

        prefix = [tag for tag in all_tags if tag.uuid.startswith(token)]
        if len(prefix) == 1:
            tag_uuid = prefix[0].uuid
            if tag_uuid not in seen:
                seen.add(tag_uuid)
                resolved.append(tag_uuid)
            continue
        if len(prefix) > 1:
            return [], f"Ambiguous tag UUID prefix: {token}"

        return [], f"Tag not found: {token}"

    return resolved, ""
