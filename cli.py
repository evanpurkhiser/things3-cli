#!/usr/bin/env python3
"""
things-cli: A command-line interface for Things 3 via the Things Cloud API.

Usage:
    python cli.py today
    python cli.py inbox
    python cli.py projects
    python cli.py areas
    python cli.py tags
    python cli.py mark <task-id> --done|--incomplete|--canceled

Requires config.py with EMAIL and PASSWORD set.
"""

import argparse
import sys
from datetime import datetime, timezone
from typing import Optional

from config import EMAIL, PASSWORD
from things_cloud.client import ThingsCloudClient
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


def fmt_task_line(task: Task, store: ThingsStore, show_project: bool = False) -> str:
    """Format a single task for terminal output."""
    parts = []

    # Checkbox
    parts.append(colored("○", DIM))

    # Title
    title = task.title or colored("(untitled)", DIM)
    parts.append(title)

    # Tags
    if task.tags:
        tag_names = [store.resolve_tag_title(t) for t in task.tags]
        parts.append(colored(" [" + ", ".join(tag_names) + "]", DIM))

    # Project
    if show_project and task.project:
        proj_title = store.resolve_project_title(task.project)
        parts.append(colored(f" · {proj_title}", DIM))

    # Deadline
    if task.deadline:
        now = datetime.now(tz=timezone.utc)
        overdue = task.deadline < now
        color = RED if overdue else YELLOW
        parts.append(colored(f" ⚑ {fmt_date(task.deadline)}", color))

    # Start date (show if overdue)
    if task.start_date:
        sr_str = fmt_date(task.start_date)
        today_str = datetime.now(tz=timezone.utc).strftime("%Y-%m-%d")
        if sr_str < today_str:
            parts.append(colored(f" (due {sr_str})", DIM))

    return " ".join(parts) if parts else title


def print_section(
    title: str, tasks: list[Task], store: ThingsStore, show_project: bool = False
):
    if not tasks:
        return
    print(colored(f"\n{title}", BOLD + CYAN))
    print(colored("─" * 40, DIM))
    for task in tasks:
        print("  " + fmt_task_line(task, store, show_project=show_project))


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_today(store: ThingsStore, args):
    """Show Today view."""
    tasks = store.today()

    if not tasks:
        print(colored("No tasks for today.", DIM))
        return

    regular = [t for t in tasks if not t.evening]
    evening = [t for t in tasks if t.evening]

    print(colored(f"★ Today  ({len(tasks)} tasks)", BOLD + YELLOW))

    if regular:
        print()
        for task in regular:
            print("  " + fmt_task_line(task, store, show_project=True))

    if evening:
        print()
        print(colored("  ☽ This Evening", BOLD + BLUE))
        print(colored("  " + "─" * 36, DIM))
        for task in evening:
            print("  " + fmt_task_line(task, store, show_project=True))


def cmd_inbox(store: ThingsStore, args):
    """Show Inbox view."""
    tasks = store.inbox()

    if not tasks:
        print(colored("Inbox is empty.", DIM))
        return

    print(colored(f"□ Inbox  ({len(tasks)} tasks)", BOLD + BLUE))
    print()
    for task in tasks:
        print("  " + fmt_task_line(task, store))


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

    # No-area projects first
    no_area = by_area.pop(None, [])
    if no_area:
        print()
        for p in no_area:
            _print_project(p, store)

    for area_uuid, area_projects in by_area.items():
        area_title = store.resolve_area_title(area_uuid) if area_uuid else "?"
        print()
        print(colored(f"  {area_title}", BOLD))
        for p in area_projects:
            _print_project(p, store, indent=4)


def _print_project(project: Task, store: ThingsStore, indent: int = 2):
    prefix = " " * indent
    title = project.title or colored("(untitled)", DIM)
    dl = colored(f" ⚑ {fmt_date(project.deadline)}", YELLOW) if project.deadline else ""
    print(f"{prefix}{colored('◎', DIM)} {title}{dl}")


def cmd_areas(store: ThingsStore, args):
    """Show all areas."""
    areas = store.areas()

    if not areas:
        print(colored("No areas.", DIM))
        return

    print(colored(f"⬡ Areas  ({len(areas)})", BOLD + MAGENTA))
    print()
    for area in areas:
        tags = ""
        if area.tags:
            tag_names = [store.resolve_tag_title(t) for t in area.tags]
            tags = colored("  [" + ", ".join(tag_names) + "]", DIM)
        print(f"  {colored('⬡', DIM)} {area.title}{tags}")


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
    for task in tasks:
        task_date = fmt_date(task.start_date)
        if task_date != current_date:
            current_date = task_date
            print()
            print(colored(f"  {task_date}", BOLD))
        print("    " + fmt_task_line(task, store, show_project=True))


def _resolve_task_identifier(
    store: ThingsStore, identifier: str
) -> tuple[Optional[Task], str]:
    ident = identifier.strip()
    if not ident:
        return None, "Missing task identifier."

    exact = store.get_task(ident)
    if exact:
        return exact, ""

    ident_lower = ident.lower()
    all_tasks = list(store.tasks(status=None, trashed=False))

    prefix_matches = [t for t in all_tasks if t.uuid.lower().startswith(ident_lower)]
    if len(prefix_matches) == 1:
        return prefix_matches[0], ""
    if len(prefix_matches) > 1:
        sample = ", ".join(
            f"{t.uuid} ({t.title or '(untitled)'})" for t in prefix_matches[:5]
        )
        return None, f"Ambiguous task id prefix. Matches: {sample}"

    title_matches = [
        t for t in all_tasks if (t.title or "").strip().lower() == ident_lower
    ]
    if len(title_matches) == 1:
        return title_matches[0], ""
    if len(title_matches) > 1:
        return None, "Multiple tasks share that exact title. Use a UUID prefix."

    return None, f"Task not found: {identifier}"


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
    """Mark one task by UUID (or unique UUID prefix)."""
    if not args.task_id:
        print(
            "Usage: python cli.py mark <task-id> --done|--incomplete|--canceled",
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

    task, err = _resolve_task_identifier(store, args.task_id)
    if not task:
        print(err, file=sys.stderr)
        return

    if task.entity != "Task6":
        print("Only Task6 tasks are supported by mark right now.", file=sys.stderr)
        return
    if task.is_project or task.is_heading:
        print("Only to-do tasks can be marked.", file=sys.stderr)
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
        print(f"Failed to mark task {action}: {e}", file=sys.stderr)
        return

    label = {
        "done": "✓ Done",
        "incomplete": "↺ Incomplete",
        "canceled": "✕ Canceled",
    }[action]
    print(colored(label, GREEN), f"{task.title}  {colored(task.uuid, DIM)}")


COMMANDS = {
    "today": cmd_today,
    "inbox": cmd_inbox,
    "projects": cmd_projects,
    "areas": cmd_areas,
    "tags": cmd_tags,
    "upcoming": cmd_upcoming,
    "mark": cmd_mark,
}


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="things-cli: Command-line interface for Things 3 via Cloud API",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="\n".join(f"  {cmd}" for cmd in COMMANDS),
    )
    parser.add_argument(
        "command",
        choices=list(COMMANDS.keys()),
        help="Command to run",
    )
    parser.add_argument(
        "task_id",
        nargs="?",
        help="Task UUID (or unique UUID prefix) for `mark`",
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

    # Disable colors if requested or if stdout is not a tty
    if args.no_color or not sys.stdout.isatty():
        global RESET, BOLD, DIM, CYAN, YELLOW, GREEN, BLUE, MAGENTA, RED
        RESET = BOLD = DIM = CYAN = YELLOW = GREEN = BLUE = MAGENTA = RED = ""

    # Fetch data
    client = ThingsCloudClient(EMAIL, PASSWORD)
    try:
        raw = client.get_all_items()
    except Exception as e:
        print(f"Error fetching data: {e}", file=sys.stderr)
        sys.exit(1)

    store = ThingsStore(raw)

    # Dispatch
    if args.command == "mark":
        COMMANDS[args.command](store, args, client)
    else:
        COMMANDS[args.command](store, args)


if __name__ == "__main__":
    main()
