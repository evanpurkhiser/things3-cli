"""Someday view command."""

import argparse

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    CYAN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    detailed_parent,
    fmt_task_line,
    fmt_task_with_note,
    fmt_project_with_note,
    _adapt_store_command,
)


def cmd_someday(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show Someday view."""
    detailed = args.detailed
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
        print(
            fmt_project_with_note(
                item, store, "  ", id_prefix_len=id_prefix_len, detailed=detailed
            )
        )

    if projects and tasks:
        print()

    for item in tasks:
        line = fmt_task_line(
            item,
            store,
            show_today_markers=False,
            id_prefix_len=id_prefix_len,
        )
        print(
            fmt_task_with_note(
                line, item, "  ", id_prefix_len=id_prefix_len, detailed=detailed
            )
        )


def register(subparsers) -> dict[str, CommandHandler]:
    subparsers.add_parser(
        "someday", help="Show the Someday view", parents=[detailed_parent]
    )
    return {"someday": _adapt_store_command(cmd_someday)}
