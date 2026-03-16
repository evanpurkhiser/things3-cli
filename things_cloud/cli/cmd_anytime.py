"""Anytime view command."""

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
    fmt_tasks_grouped,
    _adapt_store_command,
)


def cmd_anytime(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show Anytime view."""
    detailed = args.detailed
    tasks = store.anytime()

    if not tasks:
        print(colored("Anytime is empty.", DIM))
        return

    print(colored(f"{ICONS.anytime} Anytime  ({len(tasks)} tasks)", BOLD + CYAN))
    print()
    print(
        fmt_tasks_grouped(
            tasks, store, indent="  ", show_today_markers=True, detailed=detailed
        )
    )


def register(subparsers) -> dict[str, CommandHandler]:
    subparsers.add_parser(
        "anytime", help="Show the Anytime view", parents=[detailed_parent]
    )
    return {"anytime": _adapt_store_command(cmd_anytime)}
