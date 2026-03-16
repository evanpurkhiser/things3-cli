"""Edit task/project command."""

import argparse
import sys

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore
from things_cloud.schema import TaskStart
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    colored,
    fmt_task_line,
    fmt_project_line,
    _task6_note,
)


def cmd_edit(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Edit one task/project: title, container, and notes."""
    task, err, ambiguous = store.resolve_mark_identifier(args.task_id)
    if not task:
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

    update: dict = {}
    labels: list[str] = []

    if args.title is not None:
        title = args.title.strip()
        if not title:
            print("Task title cannot be empty.", file=sys.stderr)
            return
        update["tt"] = title
        labels.append("title")

    if args.notes is not None:
        update["nt"] = (
            _task6_note(args.notes)
            if args.notes
            else {"_t": "tx", "t": 1, "ch": 0, "v": ""}
        )
        labels.append("notes")

    move_raw = (args.move_target or "").strip()
    if move_raw:
        move_l = move_raw.lower()
        if move_l == "inbox":
            if task.is_project:
                print("Projects cannot be moved to Inbox.", file=sys.stderr)
                return
            update.update(
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
            if task.is_project:
                update["ar"] = []
            else:
                update.update({"pr": [], "ar": [], "agr": []})
                if task.start == TaskStart.INBOX:
                    update["st"] = TaskStart.ANYTIME
            labels.append("move=clear")
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
            if task.is_project:
                if project_uuid:
                    print(
                        "Projects can only be moved to an area or clear.",
                        file=sys.stderr,
                    )
                    return
                if area_uuid:
                    update["ar"] = [area_uuid]
                    labels.append(f"move={move_raw}")
                else:
                    print(f"Container not found: {move_raw}", file=sys.stderr)
                    return
            else:
                if project_uuid:
                    update.update({"pr": [project_uuid], "ar": [], "agr": []})
                    if task.start == TaskStart.INBOX:
                        update["st"] = TaskStart.ANYTIME
                    labels.append(f"move={move_raw}")
                elif area_uuid:
                    update.update({"ar": [area_uuid], "pr": [], "agr": []})
                    if task.start == TaskStart.INBOX:
                        update["st"] = TaskStart.ANYTIME
                    labels.append(f"move={move_raw}")
                else:
                    print(f"Container not found: {move_raw}", file=sys.stderr)
                    return

    if not update:
        print("No edit changes requested.", file=sys.stderr)
        return

    try:
        client.update_task_fields(task.uuid, update, entity=task.entity)
    except Exception as e:
        print(f"Failed to edit item: {e}", file=sys.stderr)
        return

    print(
        colored(f"{ICONS.done} Edited", GREEN),
        f"{(update.get('tt') or task.title)}  {colored(task.uuid, DIM)}",
        colored(f"({', '.join(labels)})", DIM),
    )


def register(subparsers, parents: dict) -> dict:
    edit_parser = subparsers.add_parser(
        "edit", help="Edit a task/project title, container, or notes"
    )
    edit_parser.add_argument(
        "task_id",
        help="Task/Project UUID (or unique UUID prefix)",
    )
    edit_parser.add_argument(
        "--title",
        help="Replace title",
    )
    edit_parser.add_argument(
        "--move",
        dest="move_target",
        help="Move to Inbox, clear, project UUID/prefix, or area UUID/prefix",
    )
    edit_parser.add_argument(
        "--notes",
        help="Replace notes (use empty string to clear)",
    )

    def _run_edit(store, args, client):
        cmd_edit(store, args, client)
        return None

    return {"edit": _run_edit}
