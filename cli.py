#!/usr/bin/env python3
"""
things-cli: A command-line interface for Things 3 via the Things Cloud API.

Usage:
    things3 set-auth
    things3 new <title> [--in inbox|<project/area-id>] [--when today|YYYY-MM-DD] [--before <id> | --after <id>] [--notes TEXT] [--tags tag1,tag2]
    things3 today
    things3 anytime
    things3 someday
    things3 inbox
    things3 logbook [--from YYYY-MM-DD] [--to YYYY-MM-DD]
    things3 projects
    things3 areas
    things3 tags
    things3 schedule <task-id> [--when today|someday|anytime|evening|YYYY-MM-DD]
    things3 reorder <id> --before <id> | --after <id>
    things3 edit <task-id> [--title TEXT] [--move Inbox|clear|<project/area-id>] [--notes TEXT]
    things3 mark <task-id> --done|--incomplete|--canceled
    things3 delete <id1> [<id2> ...]

"""

import argparse
import getpass
import sys
import time
import zlib
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from typing import Callable, Optional

from things_cloud.client import ThingsCloudClient
from things_cloud.auth import AuthConfigError, load_auth, write_auth
from things_cloud.ids import random_task_id
from things_cloud.log_cache import get_state_with_append_log
from things_cloud.store import ThingsStore, Task, Area, Tag
from things_cloud.schema import ENTITY_AREA, TaskProps, TaskStart, TaskStatus, TaskType

RECURRENCE_FIXED_SCHEDULE = 0
RECURRENCE_AFTER_COMPLETION = 1
LOCAL_TZ = datetime.now().astimezone().tzinfo or timezone.utc


# ---------------------------------------------------------------------------
# Formatting helpers
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

    # Entity icons
    project: str = "●"
    area: str = "▤"
    tag: str = "#"
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

    # Misc
    separator: str = "·"
    divider: str = "─"


ICONS = _Icons()


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
        now = datetime.now(tz=timezone.utc)
        overdue = task.deadline < now
        color = RED if overdue else YELLOW
        parts.append(
            colored(f" {ICONS.deadline} due by {fmt_date(task.deadline)}", color)
        )

    line = " ".join(parts) if parts else title
    if id_prefix_len and id_prefix_len > 0:
        return f"{_id_prefix(task.uuid, id_prefix_len)} {line}"
    return line


def print_section(
    title: str, tasks: list[Task], store: ThingsStore, show_project: bool = False
):
    if not tasks:
        return
    print(colored(f"\n{title}", BOLD + CYAN))
    print(colored(ICONS.divider * 40, DIM))
    for task in tasks:
        print("  " + fmt_task_line(task, store, show_project=show_project))


def print_tasks_grouped(
    tasks: list[Task],
    store: ThingsStore,
    indent: str = "  ",
    show_today_markers: bool = False,
    id_prefix_len: Optional[int] = None,
):
    """Print tasks grouped by area and project, preserving first-seen order."""
    max_group_items = 3

    def print_limited_tasks(group_tasks: list[Task], task_indent: str):
        shown = group_tasks[:max_group_items]
        for task in shown:
            print(
                task_indent
                + fmt_task_line(
                    task,
                    store,
                    show_project=False,
                    show_today_markers=show_today_markers,
                    id_prefix_len=id_prefix_len,
                )
            )
        hidden = len(group_tasks) - len(shown)
        if hidden > 0:
            print(colored(f"{task_indent}Hiding {hidden} more", DIM))

    if not tasks:
        return

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

    any_printed = False

    if unscoped:
        for task in unscoped:
            print(
                indent
                + fmt_task_line(
                    task,
                    store,
                    show_project=False,
                    show_today_markers=show_today_markers,
                    id_prefix_len=id_prefix_len,
                )
            )
        any_printed = True

    for project_uuid, project_tasks in project_only.items():
        if any_printed:
            print()
        title = store.resolve_project_title(project_uuid)
        print(
            f"{indent}{_id_prefix(project_uuid, id_prefix_len)} {colored(f'{ICONS.project} {title}', BOLD)}"
        )
        print_limited_tasks(project_tasks, indent + "  ")
        any_printed = True

    for area_uuid, area_group in by_area.items():
        if any_printed:
            print()
        area_title = store.resolve_area_title(area_uuid)
        print(
            f"{indent}{_id_prefix(area_uuid, id_prefix_len)} {colored(f'{ICONS.area} {area_title}', BOLD)}"
        )

        print_limited_tasks(area_group.tasks, indent + "  ")

        for project_uuid, project_tasks in area_group.projects.items():
            print()
            project_title = store.resolve_project_title(project_uuid)
            print(
                f"{indent}  {_id_prefix(project_uuid, id_prefix_len)} "
                + colored(f"{ICONS.project} {project_title}", BOLD)
            )
            print_limited_tasks(project_tasks, indent + "    ")
        any_printed = True


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_today(store: ThingsStore, args):
    """Show Today view."""
    tasks = store.today()
    today_items = [
        t
        for t in store.tasks(status=0, trashed=False)
        if not t.is_heading
        and t.title.strip()
        and t.entity == "Task6"
        and (t.is_today or t.evening)
    ]

    def _today_sort_key(task: Task):
        tir = task.today_index_reference or 0
        return (-tir, task.today_index, -task.index)

    today_items = sorted(today_items, key=_today_sort_key)

    if not today_items:
        print(colored("No tasks for today.", DIM))
        return

    regular = [t for t in today_items if not t.evening]
    evening = [t for t in today_items if t.evening]
    project_count = sum(1 for t in today_items if t.is_project)
    id_prefix_len = store.unique_prefix_length([item.uuid for item in today_items])

    if project_count:
        project_label = "project" if project_count == 1 else "projects"
        print(
            colored(
                f"{ICONS.today} Today  ({len(tasks)} tasks, {project_count} {project_label})",
                BOLD + YELLOW,
            )
        )
    else:
        print(colored(f"{ICONS.today} Today  ({len(tasks)} tasks)", BOLD + YELLOW))

    if regular:
        print()
        for item in regular:
            if item.is_project:
                print(
                    "  "
                    + fmt_project_line(
                        item,
                        store,
                        show_indicators=False,
                        id_prefix_len=id_prefix_len,
                    )
                )
            else:
                print(
                    "  "
                    + fmt_task_line(
                        item,
                        store,
                        show_today_markers=False,
                        id_prefix_len=id_prefix_len,
                    )
                )

    if evening:
        print()
        print(colored(f"{ICONS.evening} This Evening", BOLD + BLUE))
        print()
        for item in evening:
            if item.is_project:
                print(
                    "  "
                    + fmt_project_line(
                        item,
                        store,
                        show_indicators=False,
                        id_prefix_len=id_prefix_len,
                    )
                )
            else:
                print(
                    "  "
                    + fmt_task_line(
                        item,
                        store,
                        show_today_markers=False,
                        id_prefix_len=id_prefix_len,
                    )
                )


def cmd_inbox(store: ThingsStore, args):
    """Show Inbox view."""
    tasks = store.inbox()

    if not tasks:
        print(colored("Inbox is empty.", DIM))
        return

    print(colored(f"{ICONS.inbox} Inbox  ({len(tasks)} tasks)", BOLD + BLUE))
    print()
    print_tasks_grouped(tasks, store, indent="  ", show_today_markers=True)


def cmd_anytime(store: ThingsStore, args):
    """Show Anytime view."""
    tasks = store.anytime()

    if not tasks:
        print(colored("Anytime is empty.", DIM))
        return

    print(colored(f"{ICONS.anytime} Anytime  ({len(tasks)} tasks)", BOLD + CYAN))
    print()
    print_tasks_grouped(tasks, store, indent="  ", show_today_markers=True)


def cmd_someday(store: ThingsStore, args):
    """Show Someday view."""
    items = store.someday()

    if not items:
        print(colored("Someday is empty.", DIM))
        return

    print(colored(f"{ICONS.task_someday} Someday  ({len(items)} items)", BOLD + CYAN))
    print()
    id_prefix_len = store.unique_prefix_length([item.uuid for item in items])
    projects = [item for item in items if item.is_project]
    tasks = [item for item in items if not item.is_project]

    for item in projects:
        print("  " + fmt_project_line(item, store, id_prefix_len=id_prefix_len))

    if projects and tasks:
        print()

    for item in tasks:
        print(
            "  "
            + fmt_task_line(
                item,
                store,
                show_today_markers=False,
                id_prefix_len=id_prefix_len,
            )
        )


def cmd_projects(store: ThingsStore, args):
    """Show all active projects."""
    projects = store.projects()

    if not projects:
        print(colored("No active projects.", DIM))
        return

    print(colored(f"{ICONS.project} Projects  ({len(projects)})", BOLD + GREEN))

    # Group by area
    by_area: dict[Optional[str], list[Task]] = {}
    for p in projects:
        key = p.area
        if key not in by_area:
            by_area[key] = []
        by_area[key].append(p)

    id_scope = [p.uuid for p in projects]
    id_scope.extend(area_uuid for area_uuid in by_area.keys() if area_uuid)
    id_prefix_len = store.unique_prefix_length(id_scope)

    # No-area projects first
    no_area = by_area.pop(None, [])
    if no_area:
        print()
        for p in no_area:
            print("  " + fmt_project_line(p, store, id_prefix_len=id_prefix_len))

    for area_uuid, area_projects in by_area.items():
        area_title = store.resolve_area_title(area_uuid) if area_uuid else "?"
        print()
        area_id = _id_prefix(area_uuid, id_prefix_len) if area_uuid else "?"
        print(f"  {area_id} {colored(area_title, BOLD)}")
        for p in area_projects:
            print("    " + fmt_project_line(p, store, id_prefix_len=id_prefix_len))


def fmt_project_line(
    project: Task,
    store: ThingsStore,
    show_indicators: bool = True,
    id_prefix_len: Optional[int] = None,
) -> str:
    """Format a single project for terminal output."""
    title = project.title or colored("(untitled)", DIM)
    dl = (
        colored(f" {ICONS.deadline} {fmt_date(project.deadline)}", YELLOW)
        if project.deadline
        else ""
    )

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

    id_part = f"{_id_prefix(project.uuid, id_prefix_len)} " if id_prefix_len else ""
    return f"{id_part}{colored(marker, DIM)}{status_marker} {title}{dl}"


def cmd_areas(store: ThingsStore, args):
    """Show all areas."""
    areas = store.areas()

    if not areas:
        print(colored("No areas.", DIM))
        return

    print(colored(f"{ICONS.area} Areas  ({len(areas)})", BOLD + MAGENTA))
    print()

    id_prefix_len = store.unique_prefix_length([area.uuid for area in areas])

    for area in areas:
        tags = ""
        if area.tags:
            tag_names = [store.resolve_tag_title(t) for t in area.tags]
            tags = colored("  [" + ", ".join(tag_names) + "]", DIM)
        print(
            f"  {_id_prefix(area.uuid, id_prefix_len)} "
            f"{colored(ICONS.area, DIM)} {area.title}{tags}"
        )


def cmd_area(store: ThingsStore, args):
    """Show all projects and tasks in a specific area."""
    area, err, ambiguous = store.resolve_area_identifier(args.area_id)
    if not area:
        print(err, file=sys.stderr)
        if ambiguous:
            for match in ambiguous:
                print(f"  {ICONS.area} {match.title}  ({match.uuid})", file=sys.stderr)
        return

    show_all = getattr(args, "all", False)
    status_filter = None if show_all else 0

    # Projects in this area
    projects = [p for p in store.projects(status=status_filter) if p.area == area.uuid]
    projects.sort(key=lambda p: p.index)

    # Loose tasks (directly in area, not under a project)
    loose_tasks = [
        t
        for t in store.tasks(status=status_filter, trashed=False)
        if t.area == area.uuid
        and not t.is_project
        and not store.effective_project_uuid(t)
    ]
    loose_tasks.sort(key=lambda t: t.index)

    project_count = len(projects)
    task_count = len(loose_tasks)

    # Header
    tags = ""
    if area.tags:
        tag_names = [store.resolve_tag_title(t) for t in area.tags]
        tags = colored(" [" + ", ".join(tag_names) + "]", DIM)

    parts = []
    if project_count:
        parts.append(f"{project_count} project{'s' if project_count != 1 else ''}")
    if task_count:
        parts.append(f"{task_count} task{'s' if task_count != 1 else ''}")
    count_str = f"  ({', '.join(parts)})" if parts else ""

    print(colored(f"{ICONS.area} {area.title}{count_str}", BOLD + MAGENTA) + tags)

    all_uuids = [area.uuid] + [p.uuid for p in projects] + [t.uuid for t in loose_tasks]
    id_prefix_len = store.unique_prefix_length(all_uuids)

    # Loose tasks first
    if loose_tasks:
        print()
        for t in loose_tasks:
            print(
                "  "
                + fmt_task_line(
                    t, store, show_today_markers=True, id_prefix_len=id_prefix_len
                )
            )

    # Then projects
    if projects:
        print()
        for p in projects:
            print("  " + fmt_project_line(p, store, id_prefix_len=id_prefix_len))


def cmd_tags(store: ThingsStore, args):
    """Show all tags."""
    tags = store.tags()

    if not tags:
        print(colored("No tags.", DIM))
        return

    print(colored(f"{ICONS.tag} Tags  ({len(tags)})", BOLD))
    print()
    for tag in tags:
        shortcut = colored(f"  [{tag.shortcut}]", DIM) if tag.shortcut else ""
        print(f"  {colored(ICONS.tag, DIM)} {tag.title}{shortcut}")


def cmd_upcoming(store: ThingsStore, args):
    """Show tasks scheduled for the future."""
    now_ts = int(
        datetime.now(tz=timezone.utc)
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )

    tasks = []
    for t in store.tasks(status=0):
        if t.in_someday:
            continue
        if t.start_date is None:
            continue
        sr_ts = int(t.start_date.timestamp())
        if sr_ts > now_ts:
            tasks.append(t)

    tasks.sort(key=lambda t: t.start_date)

    if not tasks:
        print(colored("No upcoming tasks.", DIM))
        return

    print(colored(f"{ICONS.upcoming} Upcoming  ({len(tasks)} tasks)", BOLD + CYAN))

    current_date = None
    date_tasks: list[Task] = []

    def flush_date_group(day: Optional[str], grouped_tasks: list[Task]):
        if not day or not grouped_tasks:
            return
        print()
        print(colored(f"  {day}", BOLD))
        print_tasks_grouped(
            grouped_tasks,
            store,
            indent="    ",
            show_today_markers=True,
        )

    for task in tasks:
        task_date = fmt_date(task.start_date)
        if task_date != current_date:
            flush_date_group(current_date, date_tasks)
            current_date = task_date
            date_tasks = []
        date_tasks.append(task)

    flush_date_group(current_date, date_tasks)


def _parse_day(day: Optional[str], label: str) -> Optional[datetime]:
    if not day:
        return None
    try:
        parsed = datetime.strptime(day, "%Y-%m-%d")
    except ValueError:
        raise ValueError(f"Invalid {label} date: {day} (expected YYYY-MM-DD)")
    return parsed.replace(tzinfo=LOCAL_TZ)


def cmd_logbook(store: ThingsStore, args):
    """Show completed tasks, optionally filtered by completion date."""
    try:
        from_day = _parse_day(args.from_date, "--from")
        to_day = _parse_day(args.to_date, "--to")
    except ValueError as e:
        print(str(e), file=sys.stderr)
        return

    if from_day and to_day and from_day > to_day:
        print("--from date must be before or equal to --to date", file=sys.stderr)
        return

    tasks = store.logbook(from_date=from_day, to_date=to_day)
    if not tasks:
        print(colored("Logbook is empty.", DIM))
        return

    print(colored(f"{ICONS.done} Logbook  ({len(tasks)} tasks)", BOLD + GREEN))
    current_day = ""
    for task in tasks:
        day = fmt_date_local(task.stop_date)
        if day != current_day:
            print()
            print(colored(f"  {day}", BOLD))
            current_day = day
        print("    " + fmt_task_line(task, store, show_project=True))


def cmd_project(store: ThingsStore, args):
    """Show all tasks in a specific project, grouped by heading."""
    task, err, ambiguous = store.resolve_mark_identifier(args.project_id)
    if not task:
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
        return
    if not task.is_project:
        print(f"Not a project: {task.title}", file=sys.stderr)
        return

    project = task

    # Collect incomplete, non-trashed child items (tasks + headings)
    children = [
        t
        for t in store.tasks(status=None, trashed=False)
        if store.effective_project_uuid(t) == project.uuid
    ]

    # Also collect headings (store.tasks() always excludes them)
    headings = {
        t.uuid: t
        for t in store._tasks.values()
        if t.is_heading and not t.trashed and t.project == project.uuid
    }

    # Split children by heading
    ungrouped: list[Task] = []
    by_heading: dict[str, list[Task]] = {}
    for t in children:
        heading_uuid = t.action_group
        if heading_uuid and heading_uuid in headings:
            by_heading.setdefault(heading_uuid, []).append(t)
        else:
            ungrouped.append(t)

    # Sort headings by index, tasks within each group by index
    sorted_heading_uuids = sorted(
        by_heading.keys(),
        key=lambda u: headings[u].index,
    )
    ungrouped.sort(key=lambda t: t.index)
    for tasks in by_heading.values():
        tasks.sort(key=lambda t: t.index)

    total = len(children)
    progress = store.project_progress(project.uuid)
    done_count = progress.done

    # Header
    tags = ""
    if project.tags:
        tag_names = [store.resolve_tag_title(t) for t in project.tags]
        tags = colored(" [" + ", ".join(tag_names) + "]", DIM)
    print(
        colored(
            f"{ICONS.project} {project.title}  ({done_count}/{done_count + total})",
            BOLD + GREEN,
        )
        + tags
    )

    all_uuids = [project.uuid] + [t.uuid for t in children]
    id_prefix_len = store.unique_prefix_length(all_uuids)

    if not children:
        print(colored("  No tasks.", DIM))
        return

    # Ungrouped tasks first
    if ungrouped:
        print()
        for t in ungrouped:
            print(
                "  "
                + fmt_task_line(
                    t, store, show_today_markers=True, id_prefix_len=id_prefix_len
                )
            )

    # Then heading groups
    for heading_uuid in sorted_heading_uuids:
        heading = headings[heading_uuid]
        heading_tasks = by_heading[heading_uuid]
        print()
        print(colored(f"  {heading.title}", BOLD))
        for t in heading_tasks:
            print(
                "    "
                + fmt_task_line(
                    t, store, show_today_markers=True, id_prefix_len=id_prefix_len
                )
            )


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


def cmd_new(store: ThingsStore, args, client: ThingsCloudClient):
    """Create a new task with optional container, schedule, notes, and tags."""
    title = args.title.strip()
    if not title:
        print("Task title cannot be empty.", file=sys.stderr)
        return

    now_ts = time.time()
    props = asdict(TaskProps())
    props.update(
        {
            "tt": title,
            "tp": TaskType.TODO,
            "ss": TaskStatus.INCOMPLETE,
            "st": TaskStart.INBOX,
            "tr": False,
            "cd": now_ts,
            "md": now_ts,
            "nt": _task6_note(args.notes) if args.notes else None,
            "xx": {"_t": "oo", "sn": {}},
            "rmd": None,
            "rp": None,
        }
    )

    anchor = None
    anchor_id = args.before_id if args.before_id else args.after_id
    if anchor_id:
        anchor, err, ambiguous = store.resolve_task_identifier(anchor_id)
        if not anchor:
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
            return

    in_target = (args.in_target or "inbox").strip()
    if in_target.lower() != "inbox":
        project, _perr, _pamb = store.resolve_mark_identifier(in_target)
        area, _aerr, _aamb = store.resolve_area_identifier(in_target)

        project_uuid = project.uuid if project and project.is_project else None
        area_uuid = area.uuid if area else None

        if project_uuid and area_uuid:
            print(
                f"Ambiguous --in target '{in_target}' (matches project and area).",
                file=sys.stderr,
            )
            return
        if project and not project.is_project:
            print(
                "--in target must be inbox, a project ID, or an area ID.",
                file=sys.stderr,
            )
            return
        if project_uuid:
            props["pr"] = [project_uuid]
            props["st"] = TaskStart.ANYTIME
        elif area_uuid:
            props["ar"] = [area_uuid]
            props["st"] = TaskStart.ANYTIME
        else:
            print(f"Container not found: {in_target}", file=sys.stderr)
            return

    when_raw = (args.when or "").strip()
    if when_raw:
        when_l = when_raw.lower()
        if when_l == "anytime":
            props["st"] = TaskStart.ANYTIME
            props["sr"] = None
        elif when_l == "someday":
            props["st"] = TaskStart.SOMEDAY
            props["sr"] = None
        elif when_l == "today":
            day = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            props["st"] = TaskStart.ANYTIME
            props["sr"] = int(day.timestamp())
            props["tir"] = int(day.timestamp())
        else:
            try:
                day = _parse_day(when_raw, "--when")
            except ValueError as e:
                print(str(e), file=sys.stderr)
                return
            if day is None:
                print(
                    "--when requires anytime, someday, today, or YYYY-MM-DD",
                    file=sys.stderr,
                )
                return
            # Observed cloud state often models future specific dates as
            # st=Someday with sr/tir pinned to the same day.
            day_ts = int(day.timestamp())
            props["st"] = TaskStart.SOMEDAY
            props["sr"] = day_ts
            props["tir"] = day_ts

    if args.tags:
        tag_ids, tag_err = _resolve_tag_ids(store, args.tags)
        if tag_err:
            print(tag_err, file=sys.stderr)
            return
        props["tg"] = tag_ids

    def _is_today_from_props(task_props: dict) -> bool:
        if task_props.get("st") != TaskStart.ANYTIME:
            return False
        sr = task_props.get("sr")
        if sr is None:
            return False
        today_ts_local = _day_to_timestamp(
            datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
        )
        return int(sr) <= today_ts_local

    def _task_bucket(task: Task) -> tuple:
        if task.is_heading:
            return ("heading", task.project or "")
        if task.is_project:
            return ("project", task.area or "")

        project_uuid = store.effective_project_uuid(task)
        if project_uuid:
            return ("task-project", project_uuid, task.action_group or "")

        area_uuid = store.effective_area_uuid(task)
        if area_uuid:
            return ("task-area", area_uuid, task.start)

        return ("task-root", task.start)

    def _props_bucket(task_props: dict) -> tuple:
        project_uuid = None
        if task_props.get("pr"):
            project_uuid = task_props["pr"][0]
        if project_uuid:
            return ("task-project", project_uuid, "")

        area_uuid = None
        if task_props.get("ar"):
            area_uuid = task_props["ar"][0]
        if area_uuid:
            return ("task-area", area_uuid, task_props.get("st", TaskStart.INBOX))

        return ("task-root", task_props.get("st", TaskStart.INBOX))

    def _plan_ix_insert(
        ordered: list[Task],
        insert_at: int,
    ) -> tuple[int, list[tuple[str, int, str]]]:
        prev_ix = ordered[insert_at - 1].index if insert_at > 0 else None
        next_ix = ordered[insert_at].index if insert_at < len(ordered) else None
        updates: list[tuple[str, int, str]] = []

        if prev_ix is None and next_ix is None:
            return 0, updates
        if prev_ix is None:
            assert next_ix is not None
            return next_ix - 1, updates
        if next_ix is None:
            return prev_ix + 1, updates
        if prev_ix + 1 < next_ix:
            return (prev_ix + next_ix) // 2, updates

        stride = 1024
        new_index = stride
        ordered_with_new = ordered[:insert_at] + [None] + ordered[insert_at:]
        for idx, entry in enumerate(ordered_with_new, start=1):
            target_ix = idx * stride
            if entry is None:
                new_index = target_ix
                continue
            if entry.index != target_ix:
                updates.append((entry.uuid, target_ix, entry.entity))
        return new_index, updates

    def _today_sort_key(task: Task) -> tuple[int, int, int]:
        tir = task.today_index_reference or 0
        return (-tir, task.today_index, -task.index)

    today_ts = _day_to_timestamp(
        datetime.now(tz=timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    )
    new_is_today = _is_today_from_props(props)
    anchor_is_today = bool(
        anchor
        and anchor.start == TaskStart.ANYTIME
        and (anchor.is_today or anchor.evening)
    )
    target_bucket = _props_bucket(props)

    if anchor and not anchor_is_today and _task_bucket(anchor) != target_bucket:
        print(
            "Cannot place new task relative to an item in a different container/list.",
            file=sys.stderr,
        )
        return

    index_updates: list[tuple[str, int, str]] = []

    # Structural ordering (ix): always choose explicit relative placement when
    # possible; otherwise default to the top of the target list.
    siblings = [
        t
        for t in store._tasks.values()
        if not t.trashed
        and t.status == TaskStatus.INCOMPLETE
        and _task_bucket(t) == target_bucket
    ]
    siblings.sort(key=lambda t: (t.index, t.uuid))

    structural_insert_at = 0
    if anchor and _task_bucket(anchor) == target_bucket:
        anchor_pos = next(
            (i for i, t in enumerate(siblings) if t.uuid == anchor.uuid), None
        )
        if anchor_pos is None:
            print("Anchor not found in target list.", file=sys.stderr)
            return
        structural_insert_at = anchor_pos if args.before_id else anchor_pos + 1

    structural_ix, structural_updates = _plan_ix_insert(siblings, structural_insert_at)
    props["ix"] = structural_ix
    index_updates.extend(structural_updates)

    # Today ordering (ti/tir): if task lands in Today, place it relative to the
    # provided anchor when compatible, else default to top of its section.
    if new_is_today:
        section_evening = 1 if props.get("sb") else 0
        if anchor_is_today and anchor is not None:
            section_evening = 1 if anchor.evening else 0
            props["sb"] = section_evening

        today_siblings = [
            t
            for t in store._tasks.values()
            if not t.trashed
            and t.status == TaskStatus.INCOMPLETE
            and t.start == TaskStart.ANYTIME
            and (t.is_today or t.evening)
            and (1 if t.evening else 0) == section_evening
        ]
        today_siblings.sort(key=_today_sort_key)

        today_insert_at = 0
        if (
            anchor_is_today
            and anchor is not None
            and (1 if anchor.evening else 0) == section_evening
        ):
            anchor_today_pos = next(
                (i for i, t in enumerate(today_siblings) if t.uuid == anchor.uuid),
                None,
            )
            if anchor_today_pos is not None:
                today_insert_at = (
                    anchor_today_pos if args.before_id else anchor_today_pos + 1
                )

        prev_today = (
            today_siblings[today_insert_at - 1] if today_insert_at > 0 else None
        )
        next_today = (
            today_siblings[today_insert_at]
            if today_insert_at < len(today_siblings)
            else None
        )
        if next_today is not None:
            next_tir = next_today.today_index_reference or today_ts
            props["tir"] = next_tir
            props["ti"] = next_today.today_index - 1
        elif prev_today is not None:
            prev_tir = prev_today.today_index_reference or today_ts
            props["tir"] = prev_tir
            props["ti"] = prev_today.today_index + 1
        else:
            props["tir"] = today_ts
            props["ti"] = 0

    new_uuid = random_task_id()
    try:
        if anchor:
            changes = {new_uuid: {"t": 0, "e": "Task6", "p": props}}
            for task_uuid, task_index, task_entity in index_updates:
                changes[task_uuid] = {
                    "e": task_entity,
                    "p": {"ix": task_index, "md": now_ts},
                }
            client.commit(changes)
        else:
            client.create_task(new_uuid, props, entity="Task6")
    except Exception as e:
        print(f"Failed to create task: {e}", file=sys.stderr)
        return

    print(colored(f"{ICONS.done} Created", GREEN), f"{title}  {colored(new_uuid, DIM)}")


def _day_to_timestamp(day: datetime) -> int:
    return int(day.astimezone(timezone.utc).timestamp())


def cmd_schedule(store: ThingsStore, args, client: ThingsCloudClient):
    """Schedule one task/project: when/today/evening/someday/deadline."""
    task, err, ambiguous = store.resolve_mark_identifier(args.task_id)
    if not task:
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
        return

    update: dict = {}
    when_label: Optional[str] = None

    when_raw = (args.when or "").strip()
    if when_raw:
        when_l = when_raw.lower()
        if when_l == "anytime":
            update.update({"st": 1, "sr": None, "tir": None, "sb": 0})
            when_label = "anytime"
        elif when_l == "today":
            today = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            day_ts = _day_to_timestamp(today)
            update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 0})
            when_label = "today"
        elif when_l == "evening":
            today = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            day_ts = _day_to_timestamp(today)
            update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 1})
            when_label = "evening"
        elif when_l == "someday":
            update.update({"st": 2, "sr": None, "tir": None, "sb": 0})
            when_label = "someday"
        else:
            try:
                when_day = _parse_day(when_raw, "--when")
            except ValueError as e:
                print(str(e), file=sys.stderr)
                return
            assert when_day is not None
            day_ts = _day_to_timestamp(when_day)
            today_ts = _day_to_timestamp(
                datetime.now(tz=timezone.utc).replace(
                    hour=0, minute=0, second=0, microsecond=0
                )
            )
            if day_ts <= today_ts:
                update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 0})
            else:
                update.update({"st": 2, "sr": day_ts, "tir": day_ts, "sb": 0})
            when_label = f"when={when_raw}"

    if args.deadline_date:
        deadline_day = _parse_day(args.deadline_date, "--deadline")
        assert deadline_day is not None
        update["dd"] = _day_to_timestamp(deadline_day)
    if args.clear_deadline:
        update["dd"] = None

    if not update:
        print("No schedule changes requested.", file=sys.stderr)
        return

    try:
        client.update_task_fields(task.uuid, update, entity=task.entity)
    except Exception as e:
        print(f"Failed to schedule item: {e}", file=sys.stderr)
        return

    labels = []
    if "st" in update:
        labels.append(when_label or "when")
    if "dd" in update:
        labels.append(
            "deadline=none"
            if update["dd"] is None
            else f"deadline={args.deadline_date}"
        )

    print(
        colored(f"{ICONS.done} Scheduled", GREEN),
        f"{task.title}  {colored(task.uuid, DIM)}",
        colored(f"({', '.join(labels)})", DIM),
    )


def cmd_edit(store: ThingsStore, args, client: ThingsCloudClient):
    """Edit one task/project: title, container, and notes."""
    task, err, ambiguous = store.resolve_mark_identifier(args.task_id)
    if not task:
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
        return

    update: dict = {}
    labels: list[str] = []

    if args.title is not None:
        title = args.title.strip()
        if not title:
            print("Task title cannot be empty.", file=sys.stderr)
            return
        update["tt"] = title
        labels.append("title")

    if args.notes is not None:
        update["nt"] = (
            _task6_note(args.notes)
            if args.notes
            else {"_t": "tx", "t": 1, "ch": 0, "v": ""}
        )
        labels.append("notes")

    move_raw = (args.move_target or "").strip()
    if move_raw:
        move_l = move_raw.lower()
        if move_l == "inbox":
            if task.is_project:
                print("Projects cannot be moved to Inbox.", file=sys.stderr)
                return
            update.update(
                {
                    "pr": [],
                    "ar": [],
                    "agr": [],
                    "st": TaskStart.INBOX,
                    "sr": None,
                    "tir": None,
                    "sb": 0,
                }
            )
            labels.append("move=inbox")
        elif move_l == "clear":
            if task.is_project:
                update["ar"] = []
            else:
                update.update({"pr": [], "ar": [], "agr": []})
                if task.start == TaskStart.INBOX:
                    update["st"] = TaskStart.ANYTIME
            labels.append("move=clear")
        else:
            project, _perr, _pamb = store.resolve_mark_identifier(move_raw)
            area, _aerr, _aamb = store.resolve_area_identifier(move_raw)

            project_uuid = project.uuid if project and project.is_project else None
            area_uuid = area.uuid if area else None

            if project_uuid and area_uuid:
                print(
                    f"Ambiguous --move target '{move_raw}' (matches project and area).",
                    file=sys.stderr,
                )
                return
            if project and not project.is_project:
                print(
                    "--move target must be Inbox, clear, a project ID, or an area ID.",
                    file=sys.stderr,
                )
                return
            if task.is_project:
                if project_uuid:
                    print(
                        "Projects can only be moved to an area or clear.",
                        file=sys.stderr,
                    )
                    return
                if area_uuid:
                    update["ar"] = [area_uuid]
                    labels.append(f"move={move_raw}")
                else:
                    print(f"Container not found: {move_raw}", file=sys.stderr)
                    return
            else:
                if project_uuid:
                    update.update({"pr": [project_uuid], "ar": [], "agr": []})
                    if task.start == TaskStart.INBOX:
                        update["st"] = TaskStart.ANYTIME
                    labels.append(f"move={move_raw}")
                elif area_uuid:
                    update.update({"ar": [area_uuid], "pr": [], "agr": []})
                    if task.start == TaskStart.INBOX:
                        update["st"] = TaskStart.ANYTIME
                    labels.append(f"move={move_raw}")
                else:
                    print(f"Container not found: {move_raw}", file=sys.stderr)
                    return

    if not update:
        print("No edit changes requested.", file=sys.stderr)
        return

    try:
        client.update_task_fields(task.uuid, update, entity=task.entity)
    except Exception as e:
        print(f"Failed to edit item: {e}", file=sys.stderr)
        return

    print(
        colored(f"{ICONS.done} Edited", GREEN),
        f"{(update.get('tt') or task.title)}  {colored(task.uuid, DIM)}",
        colored(f"({', '.join(labels)})", DIM),
    )


def cmd_reorder(store: ThingsStore, args, client: ThingsCloudClient):
    """Reorder task/project/heading relative to another item."""
    item, err, ambiguous = store.resolve_task_identifier(args.item_id)
    if not item:
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
        return

    anchor_id = args.before_id if args.before_id else args.after_id
    anchor, err, ambiguous = store.resolve_task_identifier(anchor_id)
    if not anchor:
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
        return

    if item.uuid == anchor.uuid:
        print("Cannot reorder an item relative to itself.", file=sys.stderr)
        return

    def _is_today_orderable(task: Task) -> bool:
        return task.start == TaskStart.ANYTIME and (task.is_today or task.evening)

    today_ts = _day_to_timestamp(
        datetime.now(tz=timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    )
    is_today_reorder = _is_today_orderable(item) and _is_today_orderable(anchor)
    update: dict = {}

    if is_today_reorder:
        anchor_tir = (
            anchor.today_index_reference
            if anchor.today_index_reference is not None
            else (
                _day_to_timestamp(anchor.start_date)
                if anchor.start_date is not None
                else today_ts
            )
        )
        new_ti = anchor.today_index - 1 if args.before_id else anchor.today_index + 1
        update = {
            "tir": anchor_tir,
            "ti": new_ti,
        }
        if item.evening != anchor.evening:
            update["sb"] = 1 if anchor.evening else 0
        reorder_label = (
            f"(before={anchor.title}, today_ref={anchor_tir}, today_index={new_ti})"
            if args.before_id
            else f"(after={anchor.title}, today_ref={anchor_tir}, today_index={new_ti})"
        )
    else:

        def _bucket(task: Task) -> tuple:
            if task.is_heading:
                return ("heading", task.project or "")
            if task.is_project:
                return ("project", task.area or "")

            project_uuid = store.effective_project_uuid(task)
            if project_uuid:
                return ("task-project", project_uuid, task.action_group or "")

            area_uuid = store.effective_area_uuid(task)
            if area_uuid:
                return ("task-area", area_uuid, task.start)

            return ("task-root", task.start)

        item_bucket = _bucket(item)
        anchor_bucket = _bucket(anchor)
        if item_bucket != anchor_bucket:
            print(
                "Cannot reorder across different containers/lists.",
                file=sys.stderr,
            )
            return

        siblings = [
            t
            for t in store._tasks.values()
            if not t.trashed
            and t.status == TaskStatus.INCOMPLETE
            and _bucket(t) == item_bucket
        ]
        siblings.sort(key=lambda t: (t.index, t.uuid))

        by_uuid = {t.uuid: t for t in siblings}
        if item.uuid not in by_uuid or anchor.uuid not in by_uuid:
            print("Cannot reorder item in the selected list.", file=sys.stderr)
            return

        order = [t for t in siblings if t.uuid != item.uuid]
        anchor_pos = next(
            (i for i, t in enumerate(order) if t.uuid == anchor.uuid), None
        )
        if anchor_pos is None:
            print("Anchor not found in reorder list.", file=sys.stderr)
            return

        insert_at = anchor_pos if args.before_id else anchor_pos + 1
        order.insert(insert_at, item)

        moved_pos = next(i for i, t in enumerate(order) if t.uuid == item.uuid)
        prev_ix = order[moved_pos - 1].index if moved_pos > 0 else None
        next_ix = order[moved_pos + 1].index if moved_pos + 1 < len(order) else None

        index_updates: list[tuple[str, int, str]] = []
        if prev_ix is None and next_ix is None:
            new_index = 0
        elif prev_ix is None:
            assert next_ix is not None
            new_index = next_ix - 1
        elif next_ix is None:
            assert prev_ix is not None
            new_index = prev_ix + 1
        elif prev_ix + 1 < next_ix:
            new_index = (prev_ix + next_ix) // 2
        else:
            # No gap left between adjacent neighbors: re-pack this list to
            # restore index headroom while preserving the requested order.
            stride = 1024
            for idx, task in enumerate(order, start=1):
                target_ix = idx * stride
                if task.index != target_ix:
                    index_updates.append((task.uuid, target_ix, task.entity))
            item_reindexed = next(
                (ix for uid, ix, _ent in index_updates if uid == item.uuid),
                None,
            )
            new_index = item_reindexed if item_reindexed is not None else item.index

        if not index_updates and new_index != item.index:
            index_updates = [(item.uuid, new_index, item.entity)]

        try:
            for task_uuid, task_index, task_entity in index_updates:
                client.update_task_fields(
                    task_uuid, {"ix": task_index}, entity=task_entity
                )
        except Exception as e:
            print(f"Failed to reorder item: {e}", file=sys.stderr)
            return

        reorder_label = (
            f"(before={anchor.title}, index={new_index})"
            if args.before_id
            else f"(after={anchor.title}, index={new_index})"
        )

    if is_today_reorder:
        try:
            client.update_task_fields(item.uuid, update, entity=item.entity)
        except Exception as e:
            print(f"Failed to reorder item: {e}", file=sys.stderr)
            return

    print(
        colored(f"{ICONS.done} Reordered", GREEN),
        f"{item.title}  {colored(item.uuid, DIM)}",
        colored(reorder_label, DIM),
    )


def _validate_recurring_done(task: Task, store: ThingsStore) -> tuple[bool, str]:
    """Validate whether recurring completion can be done safely.

    Historical cloud data shows two distinct recurring completion patterns:
    - Fixed schedule templates (rr.tp=0): instance completion is typically only
      the instance mutation (`ss=3, sp=now, md=now`).
    - After completion templates (rr.tp=1): completion often couples template
      writes (`acrd`, `tir`, and sometimes `rr.ia`) in the same commit item.

    To fail closed, we only allow recurring *instances* linked to templates with
    rr.tp=0. Everything else is blocked with an explicit message.
    """
    if task.is_recurrence_template:
        return (
            False,
            "Recurring template tasks are blocked for done (template progression bookkeeping is not implemented).",
        )

    if not task.is_recurrence_instance:
        return (
            False,
            "Recurring task shape is unsupported (expected an instance with rt set and rr unset).",
        )

    if len(task.recurrence_templates) != 1:
        return (
            False,
            f"Recurring instance has {len(task.recurrence_templates)} template references; expected exactly 1.",
        )

    template_uuid = task.recurrence_templates[0]
    template = store.get_task(template_uuid)
    if not template:
        return (
            False,
            f"Recurring instance template {template_uuid} is missing from current state.",
        )

    rr = template.recurrence_rule
    if not isinstance(rr, dict):
        return (
            False,
            "Recurring instance template has unsupported recurrence rule shape (expected dict).",
        )

    rr_type = rr.get("tp")
    if rr_type == RECURRENCE_FIXED_SCHEDULE:
        return True, ""
    if rr_type == RECURRENCE_AFTER_COMPLETION:
        return (
            False,
            "Recurring 'after completion' templates (rr.tp=1) are blocked: completion requires coupled template writes (acrd/tir) not implemented yet.",
        )

    return (
        False,
        f"Recurring template type rr.tp={rr_type!r} is unsupported for safe completion.",
    )


def _validate_mark_target(task: Task, action: str, store: ThingsStore) -> str:
    """Return an error message when *task* cannot be marked for *action*."""
    if task.entity != "Task6":
        return "Only Task6 tasks are supported by mark right now."
    if task.is_heading:
        return "Headings cannot be marked."
    if task.trashed:
        return "Task is in Trash and cannot be completed."
    if action == "done" and task.status == 3:
        return "Task is already completed."
    if action == "incomplete" and task.status == 0:
        return "Task is already incomplete/open."
    if action == "canceled" and task.status == 2:
        return "Task is already canceled."
    if action == "done" and task.is_recurring:
        ok, reason = _validate_recurring_done(task, store)
        if not ok:
            return reason
    return ""


def cmd_mark(store: ThingsStore, args, client: ThingsCloudClient):
    """Mark one or more tasks/projects by UUID (or unique UUID prefix)."""
    # task_ids is a required positional and --done/--incomplete/--canceled
    # are a required mutually-exclusive group, both enforced by argparse.
    action = "done" if args.done else "incomplete" if args.incomplete else "canceled"

    targets: list[Task] = []
    seen: set[str] = set()
    for identifier in args.task_ids:
        task, err, ambiguous = store.resolve_mark_identifier(identifier)
        if not task:
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
            continue
        if task.uuid in seen:
            continue
        seen.add(task.uuid)
        targets.append(task)

    updates: list[dict] = []
    successes: list[Task] = []

    for task in targets:
        validation_error = _validate_mark_target(task, action, store)
        if validation_error:
            print(f"{validation_error} ({task.title})", file=sys.stderr)
            continue

        stop_date = time.time() if action in {"done", "canceled"} else None
        updates.append(
            {
                "task_uuid": task.uuid,
                "status": 3 if action == "done" else 0 if action == "incomplete" else 2,
                "entity": task.entity,
                "stop_date": stop_date,
            }
        )
        successes.append(task)

    if not updates:
        return

    try:
        client.set_task_statuses(updates)
    except Exception as e:
        print(f"Failed to mark items {action}: {e}", file=sys.stderr)
        return

    label = {
        "done": f"{ICONS.done} Done",
        "incomplete": f"{ICONS.incomplete} Incomplete",
        "canceled": f"{ICONS.canceled} Canceled",
    }[action]
    for task in successes:
        print(colored(label, GREEN), f"{task.title}  {colored(task.uuid, DIM)}")


def cmd_delete(store: ThingsStore, args, client: ThingsCloudClient):
    """Delete one or more tasks/projects/headings/areas by UUID/prefix."""
    targets: list[tuple[str, str, str]] = []  # (uuid, entity, title)
    seen: set[str] = set()

    for identifier in args.item_ids:
        task, task_err, task_ambiguous = store.resolve_task_identifier(identifier)
        area, area_err, area_ambiguous = store.resolve_area_identifier(identifier)

        task_match = task is not None
        area_match = area is not None

        if task_match and area_match:
            print(
                f"Ambiguous identifier '{identifier}' (matches task and area).",
                file=sys.stderr,
            )
            continue

        if not task_match and not area_match:
            if task_ambiguous and area_ambiguous:
                print(
                    f"Ambiguous identifier '{identifier}' (matches multiple tasks and areas).",
                    file=sys.stderr,
                )
            elif task_ambiguous:
                print(task_err, file=sys.stderr)
            elif area_ambiguous:
                print(area_err, file=sys.stderr)
            else:
                print(f"Item not found: {identifier}", file=sys.stderr)

            if task_ambiguous:
                id_prefix_len = store.unique_prefix_length(
                    [t.uuid for t in task_ambiguous]
                )
                for match in task_ambiguous:
                    if match.is_project:
                        print(
                            f"  {fmt_project_line(match, store, id_prefix_len=id_prefix_len)}"
                        )
                    else:
                        print(
                            f"  {fmt_task_line(match, store, show_project=True, id_prefix_len=id_prefix_len)}"
                        )
            if area_ambiguous:
                id_prefix_len = store.unique_prefix_length(
                    [a.uuid for a in area_ambiguous]
                )
                for match in area_ambiguous:
                    print(
                        f"  {_id_prefix(match.uuid, id_prefix_len)} {colored(f'{ICONS.area} {match.title}', BOLD)}"
                    )
            continue

        if task_match:
            assert task is not None
            if task.trashed:
                print(f"Item already deleted: {task.title}", file=sys.stderr)
                continue
            if task.uuid in seen:
                continue
            seen.add(task.uuid)
            targets.append((task.uuid, task.entity, task.title))
            continue

        assert area is not None
        if area.uuid in seen:
            continue
        seen.add(area.uuid)
        targets.append((area.uuid, ENTITY_AREA, area.title))

    if not targets:
        return

    updates = [
        {
            "uuid": uuid,
            "entity": entity,
        }
        for uuid, entity, _title in targets
    ]

    try:
        client.delete_items(updates)
    except Exception as e:
        print(f"Failed to delete items: {e}", file=sys.stderr)
        return

    for uuid, _entity, title in targets:
        print(
            colored(f"{ICONS.canceled} Deleted", GREEN),
            f"{title}  {colored(uuid, DIM)}",
        )


def cmd_set_auth(_args):
    """Interactively configure Things Cloud credentials."""
    print("Configure Things Cloud authentication")
    email = input("Email: ").strip()
    password = getpass.getpass("Password: ")

    try:
        path = write_auth(email, password)
    except AuthConfigError as e:
        print(f"Failed to write auth config: {e}", file=sys.stderr)
        return 1

    print(colored(f"{ICONS.done} Auth saved", GREEN), colored(str(path), DIM))
    return 0


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


def _run_mark(store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient):
    cmd_mark(store, args, client)
    return None


def _run_new(store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient):
    cmd_new(store, args, client)
    return None


def _run_schedule(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
):
    cmd_schedule(store, args, client)
    return None


def _run_reorder(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
):
    cmd_reorder(store, args, client)
    return None


def _run_edit(store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient):
    cmd_edit(store, args, client)
    return None


def _run_delete(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
):
    cmd_delete(store, args, client)
    return None


COMMANDS: dict[str, CommandHandler] = {
    "new": _run_new,
    "today": _adapt_store_command(cmd_today),
    "anytime": _adapt_store_command(cmd_anytime),
    "someday": _adapt_store_command(cmd_someday),
    "logbook": _adapt_store_command(cmd_logbook),
    "inbox": _adapt_store_command(cmd_inbox),
    "projects": _adapt_store_command(cmd_projects),
    "areas": _adapt_store_command(cmd_areas),
    "area": _adapt_store_command(cmd_area),
    "tags": _adapt_store_command(cmd_tags),
    "upcoming": _adapt_store_command(cmd_upcoming),
    "project": _adapt_store_command(cmd_project),
    "edit": _run_edit,
    "delete": _run_delete,
    "schedule": _run_schedule,
    "reorder": _run_reorder,
    "mark": _run_mark,
}

SET_AUTH_COMMAND = "set-auth"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="things3: Command-line interface for Things 3 via Cloud API",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable color output",
    )

    subparsers = parser.add_subparsers(dest="command")

    new_parser = subparsers.add_parser("new", help="Create a new task")
    new_parser.add_argument("title", help="Task title")
    new_parser.add_argument(
        "--in",
        dest="in_target",
        default="inbox",
        help="Container: inbox (default), project UUID/prefix, or area UUID/prefix",
    )
    new_parser.add_argument(
        "--when",
        help="Schedule: anytime, someday, today, or YYYY-MM-DD",
    )
    new_position_group = new_parser.add_mutually_exclusive_group()
    new_position_group.add_argument(
        "--before",
        dest="before_id",
        help="Place new task before this task/project/heading UUID/prefix",
    )
    new_position_group.add_argument(
        "--after",
        dest="after_id",
        help="Place new task after this task/project/heading UUID/prefix",
    )
    new_parser.add_argument(
        "--notes",
        default="",
        help="Task notes",
    )
    new_parser.add_argument(
        "--tags",
        help="Comma-separated tags (titles or UUID prefixes)",
    )

    # View commands (no extra args)
    subparsers.add_parser("today", help="Show the Today view (default)")
    subparsers.add_parser("anytime", help="Show the Anytime view")
    subparsers.add_parser("someday", help="Show the Someday view")
    subparsers.add_parser("inbox", help="Show the Inbox")
    logbook_parser = subparsers.add_parser("logbook", help="Show completed tasks")
    logbook_parser.add_argument(
        "--from",
        dest="from_date",
        help="Show items completed on/after this date (YYYY-MM-DD)",
    )
    logbook_parser.add_argument(
        "--to",
        dest="to_date",
        help="Show items completed on/before this date (YYYY-MM-DD)",
    )
    subparsers.add_parser("projects", help="Show all active projects")
    subparsers.add_parser("areas", help="Show all areas")
    subparsers.add_parser("tags", help="Show all tags")
    subparsers.add_parser("upcoming", help="Show tasks scheduled for the future")

    # project — takes a project identifier
    project_parser = subparsers.add_parser(
        "project", help="Show all tasks in a project"
    )
    project_parser.add_argument(
        "project_id",
        help="Project UUID (or unique UUID prefix)",
    )

    # area — takes an area identifier
    area_parser = subparsers.add_parser(
        "area", help="Show projects and tasks in an area"
    )
    area_parser.add_argument(
        "area_id",
        help="Area UUID (or unique UUID prefix)",
    )
    area_parser.add_argument(
        "--all",
        action="store_true",
        help="Include completed tasks and projects",
    )

    # mark — has its own arguments
    mark_parser = subparsers.add_parser(
        "mark", help="Mark a task done, incomplete, or canceled"
    )
    mark_parser.add_argument(
        "task_ids",
        nargs="+",
        help="Task/Project UUID(s) (or unique UUID prefixes)",
    )
    mark_group = mark_parser.add_mutually_exclusive_group(required=True)
    mark_group.add_argument(
        "--done",
        action="store_true",
        help="Set status to completed",
    )
    mark_group.add_argument(
        "--incomplete",
        action="store_true",
        help="Set status to open/incomplete",
    )
    mark_group.add_argument(
        "--canceled",
        action="store_true",
        help="Set status to canceled",
    )

    schedule_parser = subparsers.add_parser("schedule", help="Set when and deadline")
    schedule_parser.add_argument(
        "task_id",
        help="Task/Project UUID (or unique UUID prefix)",
    )
    schedule_parser.add_argument(
        "--when",
        help="Set when: today, someday, anytime, evening, or YYYY-MM-DD",
    )
    deadline_group = schedule_parser.add_mutually_exclusive_group()
    deadline_group.add_argument(
        "--deadline",
        dest="deadline_date",
        help="Set deadline date (YYYY-MM-DD)",
    )
    deadline_group.add_argument(
        "--clear-deadline",
        action="store_true",
        help="Clear existing deadline",
    )

    edit_parser = subparsers.add_parser(
        "edit", help="Edit a task/project title, container, or notes"
    )
    edit_parser.add_argument(
        "task_id",
        help="Task/Project UUID (or unique UUID prefix)",
    )
    edit_parser.add_argument(
        "--title",
        help="Replace title",
    )
    edit_parser.add_argument(
        "--move",
        dest="move_target",
        help="Move to Inbox, clear, project UUID/prefix, or area UUID/prefix",
    )
    edit_parser.add_argument(
        "--notes",
        help="Replace notes (use empty string to clear)",
    )

    delete_parser = subparsers.add_parser(
        "delete", help="Delete tasks/projects/headings/areas"
    )
    delete_parser.add_argument(
        "item_ids",
        nargs="+",
        help="Task/Project/Heading/Area UUID(s) (or unique UUID prefixes)",
    )

    reorder_parser = subparsers.add_parser(
        "reorder", help="Reorder item relative to another item"
    )
    reorder_parser.add_argument(
        "item_id",
        help="Task/Project/Heading UUID (or unique UUID prefix)",
    )
    position_group = reorder_parser.add_mutually_exclusive_group(required=True)
    position_group.add_argument(
        "--before",
        dest="before_id",
        help="Place item before this task/project/heading UUID/prefix",
    )
    position_group.add_argument(
        "--after",
        dest="after_id",
        help="Place item after this task/project/heading UUID/prefix",
    )
    # set-auth — standalone, no data fetch needed
    subparsers.add_parser(SET_AUTH_COMMAND, help="Configure Things Cloud credentials")

    args = parser.parse_args()

    # Default to today when no command is given
    if args.command is None:
        args.command = "today"

    if args.command == SET_AUTH_COMMAND:
        rc = cmd_set_auth(args)
        if rc:
            sys.exit(rc)
        return

    # Disable colors if requested or if stdout is not a tty
    if args.no_color or not sys.stdout.isatty():
        global RESET, BOLD, DIM, CYAN, YELLOW, GREEN, BLUE, MAGENTA, RED
        RESET = BOLD = DIM = CYAN = YELLOW = GREEN = BLUE = MAGENTA = RED = ""

    # Fetch data
    try:
        email, password = load_auth()
    except AuthConfigError as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)

    client = ThingsCloudClient(email, password)
    try:
        raw = get_state_with_append_log(client)
    except Exception as e:
        print(f"Error fetching data: {e}", file=sys.stderr)
        sys.exit(1)

    store = ThingsStore(raw)

    # Dispatch
    rc = COMMANDS[args.command](store, args, client)
    if rc:
        sys.exit(rc)


if __name__ == "__main__":
    main()
