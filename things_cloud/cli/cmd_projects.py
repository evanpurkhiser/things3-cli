"""Projects list and new commands."""

import argparse
import sys
import time
from datetime import datetime, timezone
from typing import Optional

from things_cloud.client import ThingsCloudClient
from things_cloud.ids import random_task_id
from things_cloud.store import ThingsStore
from things_cloud.schema import TaskType, TaskStatus, TaskStart
from things_cloud.cli.common import (
    BOLD,
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_deadline,
    _id_prefix,
    _task6_note,
    _parse_day,
    _day_to_timestamp,
    _resolve_tag_ids,
    print_project_with_note,
    _adapt_store_command,
)


def cmd_projects(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show all active projects."""
    detailed = args.detailed
    projects = store.projects()

    if not projects:
        print(colored("No active projects.", DIM))
        return

    print(colored(f"{ICONS.project} Projects  ({len(projects)})", BOLD + GREEN))

    # Group by area
    by_area: dict[Optional[str], list] = {}
    for p in projects:
        key = p.area
        if key not in by_area:
            by_area[key] = []
        by_area[key].append(p)

    id_scope = [p.uuid for p in projects]
    id_scope.extend(area_uuid for area_uuid in by_area.keys() if area_uuid)
    id_prefix_len = store.unique_prefix_length(id_scope)

    # No-area projects first
    no_area = by_area.pop(None, [])
    if no_area:
        print()
        for p in no_area:
            print_project_with_note(
                p, store, "  ", id_prefix_len=id_prefix_len, detailed=detailed
            )

    for area_uuid, area_projects in by_area.items():
        area_title = store.resolve_area_title(area_uuid) if area_uuid else "?"
        print()
        area_id = _id_prefix(area_uuid, id_prefix_len) if area_uuid else "?"
        print(f"  {area_id} {colored(area_title, BOLD)}")
        for p in area_projects:
            print_project_with_note(
                p, store, "    ", id_prefix_len=id_prefix_len, detailed=detailed
            )


def cmd_new_project(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Create a new project with optional area, tags, and when."""
    title = args.title.strip()
    if not title:
        print("Project title cannot be empty.", file=sys.stderr)
        return

    now_ts = time.time()
    props = {
        "tt": title,
        "tp": TaskType.PROJECT,
        "ss": TaskStatus.INCOMPLETE,
        "st": TaskStart.ANYTIME,
        "tr": False,
        "cd": now_ts,
        "md": now_ts,
        "nt": _task6_note(args.notes) if args.notes else None,
        "xx": {"_t": "oo", "sn": {}},
        "icp": True,
        "rmd": None,
        "rp": None,
    }

    if args.area:
        area, err, ambiguous = store.resolve_area_identifier(args.area)
        if not area:
            print(err, file=sys.stderr)
            if ambiguous:
                id_prefix_len = store.unique_prefix_length([a.uuid for a in ambiguous])
                for match in ambiguous:
                    print(
                        f"  {_id_prefix(match.uuid, id_prefix_len)} "
                        f"{colored(f'{ICONS.area} {match.title}', BOLD)}"
                    )
            return
        props["ar"] = [area.uuid]

    when_raw = (args.when or "").strip()
    if when_raw:
        when_l = when_raw.lower()
        if when_l == "anytime":
            props["st"] = TaskStart.ANYTIME
            props["sr"] = None
        elif when_l == "someday":
            props["st"] = TaskStart.SOMEDAY
            props["sr"] = None
        elif when_l == "today":
            day = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            props["st"] = TaskStart.ANYTIME
            props["sr"] = int(day.timestamp())
            props["tir"] = int(day.timestamp())
        else:
            try:
                day = _parse_day(when_raw, "--when")
            except ValueError as e:
                print(str(e), file=sys.stderr)
                return
            if day is None:
                print(
                    "--when requires anytime, someday, today, or YYYY-MM-DD",
                    file=sys.stderr,
                )
                return
            day_ts = int(day.timestamp())
            props["st"] = TaskStart.SOMEDAY
            props["sr"] = day_ts
            props["tir"] = day_ts

    if args.tags:
        tag_ids, tag_err = _resolve_tag_ids(store, args.tags)
        if tag_err:
            print(tag_err, file=sys.stderr)
            return
        props["tg"] = tag_ids

    if args.deadline_date:
        try:
            deadline_day = _parse_day(args.deadline_date, "--deadline")
        except ValueError as e:
            print(str(e), file=sys.stderr)
            return
        assert deadline_day is not None
        props["dd"] = _day_to_timestamp(deadline_day)

    new_uuid = random_task_id()
    try:
        client.create_task(new_uuid, props, entity="Task6")
    except Exception as e:
        print(f"Failed to create project: {e}", file=sys.stderr)
        return

    print(colored(f"{ICONS.done} Created", GREEN), f"{title}  {colored(new_uuid, DIM)}")


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    detailed_parent = parents["detailed"]
    projects_parser = subparsers.add_parser(
        "projects", help="Show or create projects", parents=[detailed_parent]
    )
    projects_subs = projects_parser.add_subparsers(
        dest="projects_cmd", metavar="<subcommand>"
    )
    projects_subs.add_parser(
        "list", help="Show all active projects", parents=[detailed_parent]
    )
    projects_new_parser = projects_subs.add_parser("new", help="Create a new project")
    projects_new_parser.add_argument("title", help="Project title")
    projects_new_parser.add_argument(
        "--area",
        help="Area UUID/prefix to place the project in",
    )
    projects_new_parser.add_argument(
        "--when",
        help="Schedule: anytime (default), someday, today, or YYYY-MM-DD",
    )
    projects_new_parser.add_argument(
        "--notes",
        default="",
        help="Project notes",
    )
    projects_new_parser.add_argument(
        "--tags",
        help="Comma-separated tags (titles or UUID prefixes)",
    )
    projects_new_parser.add_argument(
        "--deadline",
        dest="deadline_date",
        help="Deadline date (YYYY-MM-DD)",
    )
    # Make 'list' the default when no subcommand given
    projects_parser.set_defaults(projects_cmd="list")

    return {
        "projects": _adapt_store_command(cmd_projects),
        "projects:new": cmd_new_project,
    }
