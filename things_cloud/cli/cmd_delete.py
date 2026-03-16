"""Delete command."""

import argparse
import sys

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore
from things_cloud.schema import ENTITY_AREA
from things_cloud.cli.common import (
    BOLD,
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    fmt_project_line,
    _id_prefix,
)


def cmd_delete(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Delete one or more tasks/projects/headings/areas by UUID/prefix."""
    targets: list[tuple[str, str, str]] = []  # (uuid, entity, title)
    seen: set[str] = set()

    for identifier in args.item_ids:
        task, task_err, task_ambiguous = store.resolve_task_identifier(identifier)
        area, area_err, area_ambiguous = store.resolve_area_identifier(identifier)

        task_match = task is not None
        area_match = area is not None

        if task_match and area_match:
            print(
                f"Ambiguous identifier '{identifier}' (matches task and area).",
                file=sys.stderr,
            )
            continue

        if not task_match and not area_match:
            if task_ambiguous and area_ambiguous:
                print(
                    f"Ambiguous identifier '{identifier}' (matches multiple tasks and areas).",
                    file=sys.stderr,
                )
            elif task_ambiguous:
                print(task_err, file=sys.stderr)
            elif area_ambiguous:
                print(area_err, file=sys.stderr)
            else:
                print(f"Item not found: {identifier}", file=sys.stderr)

            if task_ambiguous:
                id_prefix_len = store.unique_prefix_length(
                    [t.uuid for t in task_ambiguous]
                )
                for match in task_ambiguous:
                    if match.is_project:
                        print(
                            f"  {fmt_project_line(match, store, id_prefix_len=id_prefix_len)}"
                        )
                    else:
                        print(
                            f"  {fmt_task_line(match, store, show_project=True, id_prefix_len=id_prefix_len)}"
                        )
            if area_ambiguous:
                id_prefix_len = store.unique_prefix_length(
                    [a.uuid for a in area_ambiguous]
                )
                for match in area_ambiguous:
                    print(
                        f"  {_id_prefix(match.uuid, id_prefix_len)} {colored(f'{ICONS.area} {match.title}', BOLD)}"
                    )
            continue

        if task_match:
            assert task is not None
            if task.trashed:
                print(f"Item already deleted: {task.title}", file=sys.stderr)
                continue
            if task.uuid in seen:
                continue
            seen.add(task.uuid)
            targets.append((task.uuid, task.entity, task.title))
            continue

        assert area is not None
        if area.uuid in seen:
            continue
        seen.add(area.uuid)
        targets.append((area.uuid, ENTITY_AREA, area.title))

    if not targets:
        return

    updates = [
        {
            "uuid": uuid,
            "entity": entity,
        }
        for uuid, entity, _title in targets
    ]

    try:
        client.delete_items(updates)
    except Exception as e:
        print(f"Failed to delete items: {e}", file=sys.stderr)
        return

    for uuid, _entity, title in targets:
        print(
            colored(f"{ICONS.deleted} Deleted", GREEN),
            f"{title}  {colored(uuid, DIM)}",
        )


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    delete_parser = subparsers.add_parser(
        "delete", help="Delete tasks/projects/headings/areas"
    )
    delete_parser.add_argument(
        "item_ids",
        nargs="+",
        help="Task/Project/Heading/Area UUID(s) (or unique UUID prefixes)",
    )

    return {"delete": cmd_delete}
