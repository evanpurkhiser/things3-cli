#!/usr/bin/env python3
"""
things-cli: A command-line interface for Things 3 via the Things Cloud API.

Usage:
    things3 set-auth
    things3 today
    things3 anytime
    things3 inbox
    things3 projects
    things3 areas
    things3 tags
    things3 mark <task-id> --done|--incomplete|--canceled

"""

import argparse
import getpass
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Callable, Optional

from things_cloud.client import ThingsCloudClient
from things_cloud.auth import AuthConfigError, load_auth, write_auth
from things_cloud.log_cache import get_state_with_append_log
from things_cloud.store import ThingsStore, Task, Area, Tag

RECURRENCE_FIXED_SCHEDULE = 0
RECURRENCE_AFTER_COMPLETION = 1


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


def _task_box(task: Task, show_someday_icon: bool = True) -> str:
    if task.is_completed:
        return "◼"
    if show_someday_icon and task.in_someday:
        return "⬚"
    return "▢"


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
    show_someday_icon: bool = True,
    id_prefix_len: Optional[int] = None,
) -> str:
    """Format a single task for terminal output."""
    parts = []

    # Checkbox
    box = _task_box(task, show_someday_icon=show_someday_icon)
    parts.append(colored(box, DIM))

    if show_today_markers:
        if task.evening:
            parts.append(colored("☽", BLUE))
        elif task.is_today:
            parts.append(colored("⭑", YELLOW))

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
        parts.append(colored(f" · {proj_title}", DIM))

    # Deadline
    if task.deadline:
        now = datetime.now(tz=timezone.utc)
        overdue = task.deadline < now
        color = RED if overdue else YELLOW
        parts.append(colored(f" ⚑ due by {fmt_date(task.deadline)}", color))

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
    print(colored("─" * 40, DIM))
    for task in tasks:
        print("  " + fmt_task_line(task, store, show_project=show_project))


def print_tasks_grouped(
    tasks: list[Task],
    store: ThingsStore,
    indent: str = "  ",
    show_today_markers: bool = False,
    show_someday_icon: bool = True,
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
                    show_someday_icon=show_someday_icon,
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
                    show_someday_icon=show_someday_icon,
                    id_prefix_len=id_prefix_len,
                )
            )
        any_printed = True

    for project_uuid, project_tasks in project_only.items():
        if any_printed:
            print()
        title = store.resolve_project_title(project_uuid)
        print(
            f"{indent}{_id_prefix(project_uuid, id_prefix_len)} {colored(f'Project: {title}', BOLD)}"
        )
        print_limited_tasks(project_tasks, indent + "  ")
        any_printed = True

    for area_uuid, area_group in by_area.items():
        if any_printed:
            print()
        area_title = store.resolve_area_title(area_uuid)
        print(
            f"{indent}{_id_prefix(area_uuid, id_prefix_len)} {colored(f'Area: {area_title}', BOLD)}"
        )

        print_limited_tasks(area_group.tasks, indent + "  ")

        for project_uuid, project_tasks in area_group.projects.items():
            print()
            project_title = store.resolve_project_title(project_uuid)
            print(
                f"{indent}  {_id_prefix(project_uuid, id_prefix_len)} "
                + colored(f"Project: {project_title}", BOLD)
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
                f"⭑ Today  ({len(tasks)} tasks, {project_count} {project_label})",
                BOLD + YELLOW,
            )
        )
    else:
        print(colored(f"⭑ Today  ({len(tasks)} tasks)", BOLD + YELLOW))

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
        print(colored("☽ This Evening", BOLD + BLUE))
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

    print(colored(f"□ Inbox  ({len(tasks)} tasks)", BOLD + BLUE))
    print()
    print_tasks_grouped(tasks, store, indent="  ", show_today_markers=True)


def cmd_anytime(store: ThingsStore, args):
    """Show Anytime view."""
    tasks = store.anytime()

    if not tasks:
        print(colored("Anytime is empty.", DIM))
        return

    print(colored(f"◌ Anytime  ({len(tasks)} tasks)", BOLD + CYAN))
    print()
    print_tasks_grouped(tasks, store, indent="  ", show_today_markers=True)


def cmd_projects(store: ThingsStore, args):
    """Show all active projects."""
    projects = store.projects()

    if not projects:
        print(colored("No active projects.", DIM))
        return

    print(colored(f"● Projects  ({len(projects)})", BOLD + GREEN))

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
    dl = colored(f" ⚑ {fmt_date(project.deadline)}", YELLOW) if project.deadline else ""

    if project.in_someday:
        marker = "◌"
    else:
        progress = store.project_progress(project.uuid)
        total = progress.total
        done = progress.done

        if total == 0 or done == 0:
            marker = "◯"
        elif done == total:
            marker = "◉"
        else:
            ratio = done / total
            if ratio < 1 / 3:
                marker = "◔"
            elif ratio < 2 / 3:
                marker = "◑"
            else:
                marker = "◕"

    status_marker = ""
    if show_indicators:
        if project.evening:
            status_marker = f" {colored('☽', BLUE)}"
        elif project.is_today:
            status_marker = f" {colored('⭑', YELLOW)}"

    id_part = f"{_id_prefix(project.uuid, id_prefix_len)} " if id_prefix_len else ""
    return f"{id_part}{colored(marker, DIM)}{status_marker} {title}{dl}"


def cmd_areas(store: ThingsStore, args):
    """Show all areas."""
    areas = store.areas()

    if not areas:
        print(colored("No areas.", DIM))
        return

    print(colored(f"⬡ Areas  ({len(areas)})", BOLD + MAGENTA))
    print()

    id_prefix_len = store.unique_prefix_length([area.uuid for area in areas])

    for area in areas:
        tags = ""
        if area.tags:
            tag_names = [store.resolve_tag_title(t) for t in area.tags]
            tags = colored("  [" + ", ".join(tag_names) + "]", DIM)
        print(
            f"  {_id_prefix(area.uuid, id_prefix_len)} "
            f"{colored('⬡', DIM)} {area.title}{tags}"
        )


def cmd_tags(store: ThingsStore, args):
    """Show all tags."""
    tags = store.tags()

    if not tags:
        print(colored("No tags.", DIM))
        return

    print(colored(f"# Tags  ({len(tags)})", BOLD))
    print()
    for tag in tags:
        shortcut = colored(f"  [{tag.shortcut}]", DIM) if tag.shortcut else ""
        print(f"  {colored('#', DIM)} {tag.title}{shortcut}")


def cmd_upcoming(store: ThingsStore, args):
    """Show tasks scheduled for the future."""
    now_ts = int(
        datetime.now(tz=timezone.utc)
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )

    tasks = []
    for t in store.tasks(status=0):
        if t.start_date is None:
            continue
        sr_ts = int(t.start_date.timestamp())
        if sr_ts > now_ts:
            tasks.append(t)

    tasks.sort(key=lambda t: t.start_date)

    if not tasks:
        print(colored("No upcoming tasks.", DIM))
        return

    print(colored(f"▷ Upcoming  ({len(tasks)} tasks)", BOLD + CYAN))

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
            show_someday_icon=False,
        )

    for task in tasks:
        task_date = fmt_date(task.start_date)
        if task_date != current_date:
            flush_date_group(current_date, date_tasks)
            current_date = task_date
            date_tasks = []
        date_tasks.append(task)

    flush_date_group(current_date, date_tasks)


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


def cmd_mark(store: ThingsStore, args, client: ThingsCloudClient):
    """Mark one task/project by UUID (or unique UUID prefix)."""
    if not args.task_id:
        print(
            "Usage: things3 mark <item-id> --done|--incomplete|--canceled",
            file=sys.stderr,
        )
        return

    selected = [
        name
        for name, enabled in (
            ("done", bool(args.done)),
            ("incomplete", bool(args.incomplete)),
            ("canceled", bool(args.canceled)),
        )
        if enabled
    ]
    if len(selected) != 1:
        print(
            "Mark requires exactly one of: --done, --incomplete, --canceled",
            file=sys.stderr,
        )
        return
    action = selected[0]

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

    if task.entity != "Task6":
        print("Only Task6 tasks are supported by mark right now.", file=sys.stderr)
        return
    if task.is_heading:
        print("Headings cannot be marked.", file=sys.stderr)
        return
    if task.trashed:
        print("Task is in Trash and cannot be completed.", file=sys.stderr)
        return
    if action == "done" and task.status == 3:
        print("Task is already completed.", file=sys.stderr)
        return
    if action == "incomplete" and task.status == 0:
        print("Task is already incomplete/open.", file=sys.stderr)
        return
    if action == "canceled" and task.status == 2:
        print("Task is already canceled.", file=sys.stderr)
        return
    if action == "done" and task.is_recurring:
        ok, reason = _validate_recurring_done(task, store)
        if not ok:
            print(reason, file=sys.stderr)
            return

    try:
        if action == "done":
            client.mark_task_done(task.uuid, entity=task.entity)
        elif action == "incomplete":
            client.mark_task_incomplete(task.uuid, entity=task.entity)
        else:
            client.mark_task_canceled(task.uuid, entity=task.entity)
    except Exception as e:
        print(f"Failed to mark item {action}: {e}", file=sys.stderr)
        return

    label = {
        "done": "✓ Done",
        "incomplete": "↺ Incomplete",
        "canceled": "✕ Canceled",
    }[action]
    print(colored(label, GREEN), f"{task.title}  {colored(task.uuid, DIM)}")


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

    print(colored("✓ Auth saved", GREEN), colored(str(path), DIM))
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


COMMANDS: dict[str, CommandHandler] = {
    "today": _adapt_store_command(cmd_today),
    "anytime": _adapt_store_command(cmd_anytime),
    "inbox": _adapt_store_command(cmd_inbox),
    "projects": _adapt_store_command(cmd_projects),
    "areas": _adapt_store_command(cmd_areas),
    "tags": _adapt_store_command(cmd_tags),
    "upcoming": _adapt_store_command(cmd_upcoming),
    "mark": _run_mark,
}

SET_AUTH_COMMAND = "set-auth"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="things3: Command-line interface for Things 3 via Cloud API",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="\n".join(f"  {cmd}" for cmd in [SET_AUTH_COMMAND, *COMMANDS]),
    )
    parser.add_argument(
        "command",
        choices=[SET_AUTH_COMMAND, *COMMANDS],
        help="Command to run",
    )
    parser.add_argument(
        "task_id",
        nargs="?",
        help="Task/Project UUID (or unique UUID prefix) for `mark`",
    )
    parser.add_argument(
        "--done",
        action="store_true",
        help="For `mark`: set status to completed",
    )
    parser.add_argument(
        "--incomplete",
        action="store_true",
        help="For `mark`: set status to open/incomplete",
    )
    parser.add_argument(
        "--canceled",
        action="store_true",
        help="For `mark`: set status to canceled",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable color output",
    )

    args = parser.parse_args()

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
