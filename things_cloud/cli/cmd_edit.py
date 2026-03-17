"""Edit task command."""

import argparse
import sys
import time

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore
from things_cloud.schema import TaskStart
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_resolve_error,
    _task6_note,
)


def cmd_edit(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Edit one or more tasks: title, notes (single only), move, tags."""
    task_ids: list[str] = args.task_ids
    multiple = len(task_ids) > 1

    # title and notes only make sense for a single task
    if multiple and args.title is not None:
        print("--title requires a single task ID.", file=sys.stderr)
        return
    if multiple and args.notes is not None:
        print("--notes requires a single task ID.", file=sys.stderr)
        return

    # Resolve all tasks up front, aborting on any error
    tasks = []
    for identifier in task_ids:
        task, err, ambiguous = store.resolve_mark_identifier(identifier)
        if not task:
            fmt_resolve_error(err, ambiguous, store)
            return
        if task.is_project:
            print("Use 'projects edit' to edit a project.", file=sys.stderr)
            return
        tasks.append(task)

    # Build the shared update (move, future tags) — validated once
    shared_update: dict = {}
    labels: list[str] = []

    move_raw = (args.move_target or "").strip()
    if move_raw:
        move_l = move_raw.lower()
        if move_l == "inbox":
            shared_update.update(
                {
                    "pr": [],
                    "ar": [],
                    "agr": [],
                    "st": TaskStart.INBOX,
                    "sr": None,
                    "tir": None,
                    "sb": 0,
                }
            )
            labels.append("move=inbox")
        elif move_l == "clear":
            labels.append("move=clear")  # applied per-task (start may differ)
        else:
            project, _perr, _pamb = store.resolve_mark_identifier(move_raw)
            area, _aerr, _aamb = store.resolve_area_identifier(move_raw)

            project_uuid = project.uuid if project and project.is_project else None
            area_uuid = area.uuid if area else None

            if project_uuid and area_uuid:
                print(
                    f"Ambiguous --move target '{move_raw}' (matches project and area).",
                    file=sys.stderr,
                )
                return
            if project and not project.is_project:
                print(
                    "--move target must be Inbox, clear, a project ID, or an area ID.",
                    file=sys.stderr,
                )
                return
            if project_uuid:
                shared_update.update({"pr": [project_uuid], "ar": [], "agr": []})
                shared_update["_move_from_inbox_st"] = TaskStart.ANYTIME
                labels.append(f"move={move_raw}")
            elif area_uuid:
                shared_update.update({"ar": [area_uuid], "pr": [], "agr": []})
                shared_update["_move_from_inbox_st"] = TaskStart.ANYTIME
                labels.append(f"move={move_raw}")
            else:
                print(f"Container not found: {move_raw}", file=sys.stderr)
                return

    # Build per-task updates (title, notes, move=clear which depends on task.start)
    now_ts = time.time()
    changes: dict = {}
    for task in tasks:
        update = dict(shared_update)

        if args.title is not None:
            title = args.title.strip()
            if not title:
                print("Task title cannot be empty.", file=sys.stderr)
                return
            update["tt"] = title
            if "title" not in labels:
                labels.append("title")

        if args.notes is not None:
            update["nt"] = (
                _task6_note(args.notes)
                if args.notes
                else {"_t": "tx", "t": 1, "ch": 0, "v": ""}
            )
            if "notes" not in labels:
                labels.append("notes")

        if move_raw.lower() == "clear":
            update.update({"pr": [], "ar": [], "agr": []})
            if task.start == TaskStart.INBOX:
                update["st"] = TaskStart.ANYTIME

        if "_move_from_inbox_st" in update:
            if task.start == TaskStart.INBOX:
                update["st"] = update.pop("_move_from_inbox_st")
            else:
                update.pop("_move_from_inbox_st")

        if not update:
            print("No edit changes requested.", file=sys.stderr)
            return

        update["md"] = now_ts
        changes[task.uuid] = {"t": 1, "e": task.entity, "p": update}

    try:
        client.commit(changes)
    except Exception as e:
        print(f"Failed to edit item: {e}", file=sys.stderr)
        return

    label_str = colored(f"({', '.join(labels)})", DIM)
    for task in tasks:
        title_display = changes[task.uuid]["p"].get("tt") or task.title
        print(
            colored(f"{ICONS.done} Edited", GREEN),
            f"{title_display}  {colored(task.uuid, DIM)}",
            label_str,
        )


def register(subparsers) -> dict[str, CommandHandler]:
    edit_parser = subparsers.add_parser(
        "edit", help="Edit a task title, container, or notes"
    )
    edit_parser.add_argument(
        "task_ids",
        nargs="+",
        help="Task UUID(s) (or unique UUID prefixes)",
    )
    edit_parser.add_argument(
        "--title",
        help="Replace title (single task only)",
    )
    edit_parser.add_argument(
        "--move",
        dest="move_target",
        help="Move to Inbox, clear, project UUID/prefix, or area UUID/prefix",
    )
    edit_parser.add_argument(
        "--notes",
        help="Replace notes (single task only; use empty string to clear)",
    )

    return {"edit": cmd_edit}
