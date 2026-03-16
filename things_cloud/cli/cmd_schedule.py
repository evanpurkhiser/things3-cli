"""Schedule command (when/deadline)."""

import argparse
import sys
from datetime import datetime, timezone
from typing import Optional

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    fmt_project_line,
    _parse_day,
    _day_to_timestamp,
)


def cmd_schedule(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Schedule one task/project: when/today/evening/someday/deadline."""
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
    when_label: Optional[str] = None

    when_raw = (args.when or "").strip()
    if when_raw:
        when_l = when_raw.lower()
        if when_l == "anytime":
            update.update({"st": 1, "sr": None, "tir": None, "sb": 0})
            when_label = "anytime"
        elif when_l == "today":
            today = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            day_ts = _day_to_timestamp(today)
            update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 0})
            when_label = "today"
        elif when_l == "evening":
            today = datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
            day_ts = _day_to_timestamp(today)
            update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 1})
            when_label = "evening"
        elif when_l == "someday":
            update.update({"st": 2, "sr": None, "tir": None, "sb": 0})
            when_label = "someday"
        else:
            try:
                when_day = _parse_day(when_raw, "--when")
            except ValueError as e:
                print(str(e), file=sys.stderr)
                return
            assert when_day is not None
            day_ts = _day_to_timestamp(when_day)
            today_ts = _day_to_timestamp(
                datetime.now(tz=timezone.utc).replace(
                    hour=0, minute=0, second=0, microsecond=0
                )
            )
            if day_ts <= today_ts:
                update.update({"st": 1, "sr": day_ts, "tir": day_ts, "sb": 0})
            else:
                update.update({"st": 2, "sr": day_ts, "tir": day_ts, "sb": 0})
            when_label = f"when={when_raw}"

    if args.deadline_date:
        deadline_day = _parse_day(args.deadline_date, "--deadline")
        assert deadline_day is not None
        update["dd"] = _day_to_timestamp(deadline_day)
    if args.clear_deadline:
        update["dd"] = None

    if not update:
        print("No schedule changes requested.", file=sys.stderr)
        return

    try:
        client.update_task_fields(task.uuid, update, entity=task.entity)
    except Exception as e:
        print(f"Failed to schedule item: {e}", file=sys.stderr)
        return

    labels = []
    if "st" in update:
        labels.append(when_label or "when")
    if "dd" in update:
        labels.append(
            "deadline=none"
            if update["dd"] is None
            else f"deadline={args.deadline_date}"
        )

    print(
        colored(f"{ICONS.done} Scheduled", GREEN),
        f"{task.title}  {colored(task.uuid, DIM)}",
        colored(f"({', '.join(labels)})", DIM),
    )


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    schedule_parser = subparsers.add_parser("schedule", help="Set when and deadline")
    schedule_parser.add_argument(
        "task_id",
        help="Task/Project UUID (or unique UUID prefix)",
    )
    schedule_parser.add_argument(
        "--when",
        help="Set when: today, someday, anytime, evening, or YYYY-MM-DD",
    )
    deadline_group = schedule_parser.add_mutually_exclusive_group()
    deadline_group.add_argument(
        "--deadline",
        dest="deadline_date",
        help="Set deadline date (YYYY-MM-DD)",
    )
    deadline_group.add_argument(
        "--clear-deadline",
        action="store_true",
        help="Clear existing deadline",
    )

    return {"schedule": cmd_schedule}
