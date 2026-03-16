"""Tags view command."""

import argparse

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    DIM,
    ICONS,
    colored,
)


def cmd_tags(store: ThingsStore, args: argparse.Namespace) -> None:
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


def register(subparsers, parents: dict) -> dict:
    subparsers.add_parser("tags", help="Show all tags")
    from things_cloud.cli.common import _adapt_store_command

    return {"tags": _adapt_store_command(cmd_tags)}
