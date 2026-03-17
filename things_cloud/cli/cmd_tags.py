"""Tags list and edit commands."""

import argparse
import sys
import time
from collections import defaultdict

from things_cloud.client import ThingsCloudClient
from things_cloud.ids import random_task_id
from things_cloud.store import ThingsStore, Tag
from things_cloud.schema import ENTITY_TAG
from things_cloud.cli.common import (
    BOLD,
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    _resolve_single_tag,
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


def cmd_edit_tag(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Edit a tag: rename or reparent."""
    tag, err = _resolve_single_tag(store, args.tag_id)
    if not tag:
        print(err, file=sys.stderr)
        return

    update: dict = {}
    labels: list[str] = []

    if args.name is not None:
        name = args.name.strip()
        if not name:
            print("Tag name cannot be empty.", file=sys.stderr)
            return
        update["tt"] = name
        labels.append("name")

    move_raw = (args.move_target or "").strip()
    if move_raw:
        if move_raw.lower() == "clear":
            update["pn"] = []
            labels.append("move=clear")
        else:
            parent, parent_err = _resolve_single_tag(store, move_raw)
            if not parent:
                print(parent_err, file=sys.stderr)
                return
            if parent.uuid == tag.uuid:
                print("A tag cannot be its own parent.", file=sys.stderr)
                return
            update["pn"] = [parent.uuid]
            labels.append(f"move={move_raw}")

    if not update:
        print("No edit changes requested.", file=sys.stderr)
        return

    try:
        client.update_task_fields(tag.uuid, update, entity=ENTITY_TAG)
    except Exception as e:
        print(f"Failed to edit tag: {e}", file=sys.stderr)
        return

    print(
        colored(f"{ICONS.done} Edited", GREEN),
        f"{(update.get('tt') or tag.title)}  {colored(tag.uuid, DIM)}",
        colored(f"({', '.join(labels)})", DIM),
    )


def cmd_new_tag(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Create a new tag with optional parent."""
    name = args.name.strip()
    if not name:
        print("Tag name cannot be empty.", file=sys.stderr)
        return

    props: dict = {
        "tt": name,
        "ix": 0,
        "xx": {"_t": "oo", "sn": {}},
    }

    if args.parent:
        parent, err = _resolve_single_tag(store, args.parent)
        if not parent:
            print(err, file=sys.stderr)
            return
        props["pn"] = [parent.uuid]

    new_uuid = random_task_id()
    try:
        client.create_task(new_uuid, props, entity=ENTITY_TAG)
    except Exception as e:
        print(f"Failed to create tag: {e}", file=sys.stderr)
        return

    print(colored(f"{ICONS.done} Created", GREEN), f"{name}  {colored(new_uuid, DIM)}")


def cmd_delete_tag(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Delete a tag by title or UUID prefix."""
    tag, err = _resolve_single_tag(store, args.tag_id)
    if not tag:
        print(err, file=sys.stderr)
        return

    try:
        client.delete_items([{"uuid": tag.uuid, "entity": ENTITY_TAG}])
    except Exception as e:
        print(f"Failed to delete tag: {e}", file=sys.stderr)
        return

    print(
        colored(f"{ICONS.deleted} Deleted", GREEN),
        f"{tag.title}  {colored(tag.uuid, DIM)}",
    )


def register(subparsers) -> dict[str, CommandHandler]:
    tags_parser = subparsers.add_parser("tags", help="Show or edit tags")
    tags_subs = tags_parser.add_subparsers(dest="tags_cmd", metavar="<subcommand>")
    tags_subs.add_parser("list", help="Show all tags")
    tags_new_parser = tags_subs.add_parser("new", help="Create a new tag")
    tags_new_parser.add_argument("name", help="Tag name")
    tags_new_parser.add_argument(
        "--parent",
        help="Parent tag (title or UUID prefix)",
    )
    tags_edit_parser = tags_subs.add_parser("edit", help="Rename or reparent a tag")
    tags_edit_parser.add_argument(
        "tag_id",
        help="Tag title or UUID prefix",
    )
    tags_edit_parser.add_argument(
        "--name",
        help="New tag name",
    )
    tags_edit_parser.add_argument(
        "--move",
        dest="move_target",
        help="Reparent to another tag (by title or UUID prefix), or 'clear' to remove parent",
    )
    tags_delete_parser = tags_subs.add_parser("delete", help="Delete a tag")
    tags_delete_parser.add_argument(
        "tag_id",
        help="Tag title or UUID prefix",
    )
    tags_parser.set_defaults(tags_cmd="list")

    return {
        "tags": _adapt_store_command(cmd_tags),
        "tags:new": cmd_new_tag,
        "tags:edit": cmd_edit_tag,
        "tags:delete": cmd_delete_tag,
    }
