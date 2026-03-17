"""Tags view command."""

import argparse
from collections import defaultdict

from things_cloud.store import ThingsStore, Tag
from things_cloud.cli.common import (
    BOLD,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    _adapt_store_command,
)


def cmd_tags(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show all tags, with subtags nested under their parent."""
    tags = store.tags()

    if not tags:
        print(colored("No tags.", DIM))
        return

    print(colored(f"{ICONS.tag} Tags  ({len(tags)})", BOLD))
    print()

    by_uuid: dict[str, Tag] = {t.uuid: t for t in tags}
    children: dict[str, list[Tag]] = defaultdict(list)
    top_level: list[Tag] = []

    for tag in tags:
        if tag.parent_uuid and tag.parent_uuid in by_uuid:
            children[tag.parent_uuid].append(tag)
        else:
            top_level.append(tag)

    def _shortcut(tag: Tag) -> str:
        return colored(f"  [{tag.shortcut}]", DIM) if tag.shortcut else ""

    def _print_subtags(subtags: list[Tag], indent: str) -> None:
        """Recursively print a list of sibling tags at the given indent level."""
        for i, tag in enumerate(subtags):
            is_last = i == len(subtags) - 1
            connector = colored("└╴" if is_last else "├╴", DIM)
            print(
                f"  {indent}{connector}{colored(ICONS.tag, DIM)} {tag.title}{_shortcut(tag)}"
            )
            grandchildren = children.get(tag.uuid, [])
            if grandchildren:
                child_indent = indent + ("  " if is_last else colored("│", DIM) + " ")
                _print_subtags(grandchildren, child_indent)

    for tag in top_level:
        print(f"  {colored(ICONS.tag, DIM)} {tag.title}{_shortcut(tag)}")
        subtags = children.get(tag.uuid, [])
        if subtags:
            _print_subtags(subtags, "")


def register(subparsers) -> dict[str, CommandHandler]:
    subparsers.add_parser("tags", help="Show all tags")
    return {"tags": _adapt_store_command(cmd_tags)}
