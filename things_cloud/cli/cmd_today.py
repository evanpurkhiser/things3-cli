"""Today view command."""

import argparse

from things_cloud.store import ThingsStore, Task
from things_cloud.cli.common import (
    BOLD,
    BLUE,
    YELLOW,
    DIM,
    ICONS,
    colored,
    fmt_task_line,
    print_task_with_note,
    print_project_with_note,
)


def cmd_today(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show Today view."""
    detailed = args.detailed
    tasks = store.today()
    today_items = [
        t
        for t in store.tasks(status=0, trashed=False)
        if not t.is_heading
        and t.title.strip()
        and t.entity == "Task6"
        and (t.is_today or t.evening)
    ]

    def _today_sort_key(task: Task):
        tir = task.today_index_reference or 0
        return (-tir, task.today_index, -task.index)

    today_items = sorted(today_items, key=_today_sort_key)

    if not today_items:
        print(colored("No tasks for today.", DIM))
        return

    regular = [t for t in today_items if not t.evening]
    evening = [t for t in today_items if t.evening]
    project_count = sum(1 for t in today_items if t.is_project)
    id_prefix_len = store.unique_prefix_length([item.uuid for item in today_items])

    if project_count:
        project_label = "project" if project_count == 1 else "projects"
        print(
            colored(
                f"{ICONS.today} Today  ({len(tasks)} tasks, {project_count} {project_label})",
                BOLD + YELLOW,
            )
        )
    else:
        print(colored(f"{ICONS.today} Today  ({len(tasks)} tasks)", BOLD + YELLOW))

    if regular:
        print()
        for item in regular:
            if item.is_project:
                print_project_with_note(
                    item,
                    store,
                    "  ",
                    show_indicators=False,
                    id_prefix_len=id_prefix_len,
                    detailed=detailed,
                )
            else:
                line = fmt_task_line(
                    item,
                    store,
                    show_today_markers=False,
                    id_prefix_len=id_prefix_len,
                )
                print_task_with_note(
                    line, item, "  ", id_prefix_len=id_prefix_len, detailed=detailed
                )

    if evening:
        print()
        print(colored(f"{ICONS.evening} This Evening", BOLD + BLUE))
        print()
        for item in evening:
            if item.is_project:
                print_project_with_note(
                    item,
                    store,
                    "  ",
                    show_indicators=False,
                    id_prefix_len=id_prefix_len,
                    detailed=detailed,
                )
            else:
                line = fmt_task_line(
                    item,
                    store,
                    show_today_markers=False,
                    id_prefix_len=id_prefix_len,
                )
                print_task_with_note(
                    line, item, "  ", id_prefix_len=id_prefix_len, detailed=detailed
                )


def register(subparsers, parents: dict) -> dict:
    detailed_parent = parents["detailed"]
    subparsers.add_parser(
        "today", help="Show the Today view (default)", parents=[detailed_parent]
    )
    from things_cloud.cli.common import _adapt_store_command

    return {"today": _adapt_store_command(cmd_today)}
