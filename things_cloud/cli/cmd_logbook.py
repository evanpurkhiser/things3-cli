"""Logbook view command."""

import argparse
import sys

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    fmt_date_local,
    print_task_with_note,
    _parse_day,
    _adapt_store_command,
)


def cmd_logbook(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show completed tasks, optionally filtered by completion date."""
    detailed = args.detailed
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
        line = fmt_task_line(task, store, show_project=True)
        print_task_with_note(line, task, "    ", detailed=detailed)


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    detailed_parent = parents["detailed"]
    logbook_parser = subparsers.add_parser(
        "logbook", help="Show the Logbook", parents=[detailed_parent]
    )
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
    return {"logbook": _adapt_store_command(cmd_logbook)}
