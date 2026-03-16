"""Areas list and new commands."""

import argparse
import sys
import time

from things_cloud.client import ThingsCloudClient
from things_cloud.ids import random_task_id
from things_cloud.store import ThingsStore
from things_cloud.schema import ENTITY_AREA
from things_cloud.cli.common import (
    BOLD,
    MAGENTA,
    GREEN,
    DIM,
    ICONS,
    colored,
    _id_prefix,
)


def cmd_areas(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show all areas."""
    areas = store.areas()

    if not areas:
        print(colored("No areas.", DIM))
        return

    print(colored(f"{ICONS.area} Areas  ({len(areas)})", BOLD + MAGENTA))
    print()

    id_prefix_len = store.unique_prefix_length([area.uuid for area in areas])

    for area in areas:
        tags = ""
        if area.tags:
            tag_names = [store.resolve_tag_title(t) for t in area.tags]
            tags = colored("  [" + ", ".join(tag_names) + "]", DIM)
        print(
            f"  {_id_prefix(area.uuid, id_prefix_len)} "
            f"{colored(ICONS.area, DIM)} {area.title}{tags}"
        )


def cmd_new_area(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Create a new area with just a title."""
    title = args.title.strip()
    if not title:
        print("Area title cannot be empty.", file=sys.stderr)
        return

    now_ts = time.time()
    props = {
        "tt": title,
        "ix": 0,
        "xx": {"_t": "oo", "sn": {}},
        "cd": now_ts,
        "md": now_ts,
    }

    new_uuid = random_task_id()
    try:
        client.create_task(new_uuid, props, entity=ENTITY_AREA)
    except Exception as e:
        print(f"Failed to create area: {e}", file=sys.stderr)
        return

    print(colored(f"{ICONS.done} Created", GREEN), f"{title}  {colored(new_uuid, DIM)}")


def register(subparsers, parents: dict) -> dict:
    areas_parser = subparsers.add_parser("areas", help="Show or create areas")
    areas_subs = areas_parser.add_subparsers(dest="areas_cmd", metavar="<subcommand>")
    areas_subs.add_parser("list", help="Show all areas")
    areas_new_parser = areas_subs.add_parser("new", help="Create a new area")
    areas_new_parser.add_argument("title", help="Area title")
    # Make 'list' the default when no subcommand given
    areas_parser.set_defaults(areas_cmd="list")

    from things_cloud.cli.common import _adapt_store_command

    def _run_new_area(store, args, client):
        cmd_new_area(store, args, client)
        return None

    return {
        "areas": _adapt_store_command(cmd_areas),
        "areas:new": _run_new_area,
    }
