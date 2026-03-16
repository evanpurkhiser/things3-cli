"""Upcoming view command."""

import argparse
from datetime import datetime, timezone
from typing import Optional

from things_cloud.store import ThingsStore, Task
from things_cloud.cli.common import (
    BOLD,
    CYAN,
    DIM,
    ICONS,
    colored,
    fmt_date,
    print_tasks_grouped,
    _day_to_timestamp,
)


def cmd_upcoming(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show tasks scheduled for the future."""
    detailed = args.detailed
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
            detailed=detailed,
        )

    for task in tasks:
        task_date = fmt_date(task.start_date)
        if task_date != current_date:
            flush_date_group(current_date, date_tasks)
            current_date = task_date
            date_tasks = []
        date_tasks.append(task)

    flush_date_group(current_date, date_tasks)


def register(subparsers, parents: dict) -> dict:
    detailed_parent = parents["detailed"]
    subparsers.add_parser(
        "upcoming",
        help="Show tasks scheduled for the future",
        parents=[detailed_parent],
    )
    from things_cloud.cli.common import _adapt_store_command

    return {"upcoming": _adapt_store_command(cmd_upcoming)}
