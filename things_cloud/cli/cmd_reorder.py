"""Reorder command."""

import argparse
import sys
from datetime import datetime, timezone

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore, Task
from things_cloud.schema import TaskStart, TaskStatus
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    fmt_project_line,
    _day_to_timestamp,
)


def cmd_reorder(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Reorder task/project/heading relative to another item."""
    item, err, ambiguous = store.resolve_task_identifier(args.item_id)
    if not item:
        print(err, file=sys.stderr)
        if ambiguous:
            id_prefix_len = store.unique_prefix_length([t.uuid for t in ambiguous])
            for match in ambiguous:
                if match.is_project:
                    print(
                        f"  {fmt_project_line(match, store, id_prefix_len=id_prefix_len)}"
                    )
                else:
                    print(
                        f"  {fmt_task_line(match, store, show_project=True, id_prefix_len=id_prefix_len)}"
                    )
        return

    anchor_id = args.before_id if args.before_id else args.after_id
    anchor, err, ambiguous = store.resolve_task_identifier(anchor_id)
    if not anchor:
        print(err, file=sys.stderr)
        if ambiguous:
            id_prefix_len = store.unique_prefix_length([t.uuid for t in ambiguous])
            for match in ambiguous:
                if match.is_project:
                    print(
                        f"  {fmt_project_line(match, store, id_prefix_len=id_prefix_len)}"
                    )
                else:
                    print(
                        f"  {fmt_task_line(match, store, show_project=True, id_prefix_len=id_prefix_len)}"
                    )
        return

    if item.uuid == anchor.uuid:
        print("Cannot reorder an item relative to itself.", file=sys.stderr)
        return

    def _is_today_orderable(task: Task) -> bool:
        return task.start == TaskStart.ANYTIME and (task.is_today or task.evening)

    today_ts = _day_to_timestamp(
        datetime.now(tz=timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    )
    is_today_reorder = _is_today_orderable(item) and _is_today_orderable(anchor)
    update: dict = {}

    if is_today_reorder:
        anchor_tir = (
            anchor.today_index_reference
            if anchor.today_index_reference is not None
            else (
                _day_to_timestamp(anchor.start_date)
                if anchor.start_date is not None
                else today_ts
            )
        )
        new_ti = anchor.today_index - 1 if args.before_id else anchor.today_index + 1
        update = {
            "tir": anchor_tir,
            "ti": new_ti,
        }
        if item.evening != anchor.evening:
            update["sb"] = 1 if anchor.evening else 0
        reorder_label = (
            f"(before={anchor.title}, today_ref={anchor_tir}, today_index={new_ti})"
            if args.before_id
            else f"(after={anchor.title}, today_ref={anchor_tir}, today_index={new_ti})"
        )
    else:

        def _bucket(task: Task) -> tuple:
            if task.is_heading:
                return ("heading", task.project or "")
            if task.is_project:
                return ("project", task.area or "")

            project_uuid = store.effective_project_uuid(task)
            if project_uuid:
                return ("task-project", project_uuid, task.action_group or "")

            area_uuid = store.effective_area_uuid(task)
            if area_uuid:
                return ("task-area", area_uuid, task.start)

            return ("task-root", task.start)

        item_bucket = _bucket(item)
        anchor_bucket = _bucket(anchor)
        if item_bucket != anchor_bucket:
            print(
                "Cannot reorder across different containers/lists.",
                file=sys.stderr,
            )
            return

        siblings = [
            t
            for t in store._tasks.values()
            if not t.trashed
            and t.status == TaskStatus.INCOMPLETE
            and _bucket(t) == item_bucket
        ]
        siblings.sort(key=lambda t: (t.index, t.uuid))

        by_uuid = {t.uuid: t for t in siblings}
        if item.uuid not in by_uuid or anchor.uuid not in by_uuid:
            print("Cannot reorder item in the selected list.", file=sys.stderr)
            return

        order = [t for t in siblings if t.uuid != item.uuid]
        anchor_pos = next(
            (i for i, t in enumerate(order) if t.uuid == anchor.uuid), None
        )
        if anchor_pos is None:
            print("Anchor not found in reorder list.", file=sys.stderr)
            return

        insert_at = anchor_pos if args.before_id else anchor_pos + 1
        order.insert(insert_at, item)

        moved_pos = next(i for i, t in enumerate(order) if t.uuid == item.uuid)
        prev_ix = order[moved_pos - 1].index if moved_pos > 0 else None
        next_ix = order[moved_pos + 1].index if moved_pos + 1 < len(order) else None

        index_updates: list[tuple[str, int, str]] = []
        if prev_ix is None and next_ix is None:
            new_index = 0
        elif prev_ix is None:
            assert next_ix is not None
            new_index = next_ix - 1
        elif next_ix is None:
            assert prev_ix is not None
            new_index = prev_ix + 1
        elif prev_ix + 1 < next_ix:
            new_index = (prev_ix + next_ix) // 2
        else:
            # No gap left between adjacent neighbors: re-pack this list to
            # restore index headroom while preserving the requested order.
            stride = 1024
            for idx, task in enumerate(order, start=1):
                target_ix = idx * stride
                if task.index != target_ix:
                    index_updates.append((task.uuid, target_ix, task.entity))
            item_reindexed = next(
                (ix for uid, ix, _ent in index_updates if uid == item.uuid),
                None,
            )
            new_index = item_reindexed if item_reindexed is not None else item.index

        if not index_updates and new_index != item.index:
            index_updates = [(item.uuid, new_index, item.entity)]

        try:
            for task_uuid, task_index, task_entity in index_updates:
                client.update_task_fields(
                    task_uuid, {"ix": task_index}, entity=task_entity
                )
        except Exception as e:
            print(f"Failed to reorder item: {e}", file=sys.stderr)
            return

        reorder_label = (
            f"(before={anchor.title}, index={new_index})"
            if args.before_id
            else f"(after={anchor.title}, index={new_index})"
        )

    if is_today_reorder:
        try:
            client.update_task_fields(item.uuid, update, entity=item.entity)
        except Exception as e:
            print(f"Failed to reorder item: {e}", file=sys.stderr)
            return

    print(
        colored(f"{ICONS.done} Reordered", GREEN),
        f"{item.title}  {colored(item.uuid, DIM)}",
        colored(reorder_label, DIM),
    )


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    reorder_parser = subparsers.add_parser(
        "reorder", help="Reorder item relative to another item"
    )
    reorder_parser.add_argument(
        "item_id",
        help="Task/Project/Heading UUID (or unique UUID prefix)",
    )
    position_group = reorder_parser.add_mutually_exclusive_group(required=True)
    position_group.add_argument(
        "--before",
        dest="before_id",
        help="Place item before this task/project/heading UUID/prefix",
    )
    position_group.add_argument(
        "--after",
        dest="after_id",
        help="Place item after this task/project/heading UUID/prefix",
    )

    return {"reorder": cmd_reorder}
