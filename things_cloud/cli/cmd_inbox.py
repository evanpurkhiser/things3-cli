"""Inbox view command."""

import argparse

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    BLUE,
    DIM,
    ICONS,
    colored,
    print_tasks_grouped,
)


def cmd_inbox(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show Inbox view."""
    detailed = args.detailed
    tasks = store.inbox()

    if not tasks:
        print(colored("Inbox is empty.", DIM))
        return

    print(colored(f"{ICONS.inbox} Inbox  ({len(tasks)} tasks)", BOLD + BLUE))
    print()
    print_tasks_grouped(
        tasks, store, indent="  ", show_today_markers=True, detailed=detailed
    )


def register(subparsers, parents: dict) -> dict:
    detailed_parent = parents["detailed"]
    subparsers.add_parser("inbox", help="Show the Inbox", parents=[detailed_parent])
    from things_cloud.cli.common import _adapt_store_command

    return {"inbox": _adapt_store_command(cmd_inbox)}
