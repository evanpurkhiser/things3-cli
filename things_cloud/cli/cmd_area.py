"""Single area detail view command."""

import argparse
import sys

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    MAGENTA,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    print_task_with_note,
    print_project_with_note,
    _adapt_store_command,
)


def cmd_area(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show all projects and tasks in a specific area."""
    detailed = args.detailed
    area, err, ambiguous = store.resolve_area_identifier(args.area_id)
    if not area:
        print(err, file=sys.stderr)
        if ambiguous:
            for match in ambiguous:
                print(f"  {ICONS.area} {match.title}  ({match.uuid})", file=sys.stderr)
        return

    show_all = getattr(args, "all", False)
    status_filter = None if show_all else 0

    # Projects in this area
    projects = [p for p in store.projects(status=status_filter) if p.area == area.uuid]
    projects.sort(key=lambda p: p.index)

    # Loose tasks (directly in area, not under a project)
    loose_tasks = [
        t
        for t in store.tasks(status=status_filter, trashed=False)
        if t.area == area.uuid
        and not t.is_project
        and not store.effective_project_uuid(t)
    ]
    loose_tasks.sort(key=lambda t: t.index)

    project_count = len(projects)
    task_count = len(loose_tasks)

    # Header
    tags = ""
    if area.tags:
        tag_names = [store.resolve_tag_title(t) for t in area.tags]
        tags = colored(" [" + ", ".join(tag_names) + "]", DIM)

    parts = []
    if project_count:
        parts.append(f"{project_count} project{'s' if project_count != 1 else ''}")
    if task_count:
        parts.append(f"{task_count} task{'s' if task_count != 1 else ''}")
    count_str = f"  ({', '.join(parts)})" if parts else ""

    print(colored(f"{ICONS.area} {area.title}{count_str}", BOLD + MAGENTA) + tags)

    all_uuids = [area.uuid] + [p.uuid for p in projects] + [t.uuid for t in loose_tasks]
    id_prefix_len = store.unique_prefix_length(all_uuids)

    # Loose tasks first
    if loose_tasks:
        print()
        for t in loose_tasks:
            line = fmt_task_line(
                t, store, show_today_markers=True, id_prefix_len=id_prefix_len
            )
            print_task_with_note(
                line,
                t,
                "  ",
                show_today_markers=True,
                id_prefix_len=id_prefix_len,
                detailed=detailed,
            )

    # Then projects
    if projects:
        print()
        for p in projects:
            print_project_with_note(
                p, store, "  ", id_prefix_len=id_prefix_len, detailed=detailed
            )


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    detailed_parent = parents["detailed"]
    area_parser = subparsers.add_parser(
        "area", help="Show projects and tasks in an area", parents=[detailed_parent]
    )
    area_parser.add_argument(
        "area_id",
        help="Area UUID (or unique UUID prefix)",
    )
    area_parser.add_argument(
        "--all",
        action="store_true",
        help="Include completed tasks and projects",
    )
    return {"area": _adapt_store_command(cmd_area)}
