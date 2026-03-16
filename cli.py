#!/usr/bin/env python3

import argparse
import sys

from things_cloud.client import ThingsCloudClient
from things_cloud.auth import AuthConfigError, load_auth
from things_cloud.log_cache import get_state_with_append_log, fold_state_from_append_log
from things_cloud.dirs import append_log_dir
from things_cloud.store import ThingsStore

import things_cloud.cli.cmd_inbox as _cmd_inbox
import things_cloud.cli.cmd_today as _cmd_today
import things_cloud.cli.cmd_upcoming as _cmd_upcoming
import things_cloud.cli.cmd_anytime as _cmd_anytime
import things_cloud.cli.cmd_someday as _cmd_someday
import things_cloud.cli.cmd_logbook as _cmd_logbook
import things_cloud.cli.cmd_projects as _cmd_projects
import things_cloud.cli.cmd_areas as _cmd_areas
import things_cloud.cli.cmd_tags as _cmd_tags
import things_cloud.cli.cmd_project as _cmd_project
import things_cloud.cli.cmd_area as _cmd_area
import things_cloud.cli.cmd_new as _cmd_new
import things_cloud.cli.cmd_edit as _cmd_edit
import things_cloud.cli.cmd_mark as _cmd_mark
import things_cloud.cli.cmd_schedule as _cmd_schedule
import things_cloud.cli.cmd_reorder as _cmd_reorder
import things_cloud.cli.cmd_delete as _cmd_delete
import things_cloud.cli.cmd_set_auth as _cmd_set_auth

# ---------------------------------------------------------------------------
# Re-exports for backward compatibility (tests import these as cli.cmd_X)
# ---------------------------------------------------------------------------

cmd_inbox = _cmd_inbox.cmd_inbox
cmd_today = _cmd_today.cmd_today
cmd_upcoming = _cmd_upcoming.cmd_upcoming
cmd_anytime = _cmd_anytime.cmd_anytime
cmd_someday = _cmd_someday.cmd_someday
cmd_logbook = _cmd_logbook.cmd_logbook
cmd_projects = _cmd_projects.cmd_projects
cmd_new_project = _cmd_projects.cmd_new_project
cmd_areas = _cmd_areas.cmd_areas
cmd_new_area = _cmd_areas.cmd_new_area
cmd_tags = _cmd_tags.cmd_tags
cmd_project = _cmd_project.cmd_project
cmd_area = _cmd_area.cmd_area
cmd_new = _cmd_new.cmd_new
cmd_edit = _cmd_edit.cmd_edit
cmd_mark = _cmd_mark.cmd_mark
cmd_schedule = _cmd_schedule.cmd_schedule
cmd_reorder = _cmd_reorder.cmd_reorder
cmd_delete = _cmd_delete.cmd_delete
cmd_set_auth = _cmd_set_auth.cmd_set_auth

SET_AUTH_COMMAND = _cmd_set_auth.SET_AUTH_COMMAND

# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

_MODULES = [
    _cmd_inbox,
    _cmd_today,
    _cmd_upcoming,
    _cmd_anytime,
    _cmd_someday,
    _cmd_logbook,
    _cmd_projects,
    _cmd_areas,
    _cmd_tags,
    _cmd_project,
    _cmd_area,
    _cmd_new,
    _cmd_edit,
    _cmd_mark,
    _cmd_schedule,
    _cmd_reorder,
    _cmd_delete,
    _cmd_set_auth,
]


def main():
    parser = argparse.ArgumentParser(
        description="things3: Command-line interface for Things 3 via Cloud API",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable color output",
    )
    parser.add_argument(
        "--no-sync",
        action="store_true",
        help="Skip cloud sync and use local cache only",
    )

    subparsers = parser.add_subparsers(dest="command", metavar="<command>")

    # Shared parent parser for view commands that show tasks
    detailed_parent = argparse.ArgumentParser(add_help=False)
    detailed_parent.add_argument(
        "--detailed",
        action="store_true",
        help="Show notes beneath each task",
    )

    parents = {"detailed": detailed_parent}

    COMMANDS = {}
    for module in _MODULES:
        COMMANDS.update(module.register(subparsers, parents))

    args = parser.parse_args()

    # Default to today when no command is given
    if args.command is None:
        args.command = "today"

    if args.command == SET_AUTH_COMMAND:
        rc = cmd_set_auth(args)
        if rc:
            sys.exit(rc)
        return

    # Disable colors if requested or if stdout is not a tty
    if args.no_color or not sys.stdout.isatty():
        import things_cloud.cli.common as _common

        _common.RESET = _common.BOLD = _common.DIM = ""
        _common.CYAN = _common.YELLOW = _common.GREEN = ""
        _common.BLUE = _common.MAGENTA = _common.RED = ""

    # Fetch data
    try:
        email, password = load_auth()
    except AuthConfigError as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)

    client = ThingsCloudClient(email, password)
    try:
        if args.no_sync:
            raw = fold_state_from_append_log(append_log_dir())
        else:
            raw = get_state_with_append_log(client)
    except Exception as e:
        print(f"Error fetching data: {e}", file=sys.stderr)
        sys.exit(1)

    store = ThingsStore(raw)

    # Dispatch: handle nested subcommands (projects new, areas new)
    command_key = args.command
    if args.command == "projects":
        sub = getattr(args, "projects_cmd", "list")
        if sub and sub != "list":
            command_key = f"projects:{sub}"
    elif args.command == "areas":
        sub = getattr(args, "areas_cmd", "list")
        if sub and sub != "list":
            command_key = f"areas:{sub}"

    rc = COMMANDS[command_key](store, args, client)
    if rc:
        sys.exit(rc)


if __name__ == "__main__":
    main()
