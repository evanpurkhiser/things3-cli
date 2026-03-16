"""New task creation command."""

import argparse
import sys
import time
from dataclasses import asdict
from datetime import datetime, timezone

from things_cloud.client import ThingsCloudClient
from things_cloud.ids import random_task_id
from things_cloud.store import ThingsStore, Task
from things_cloud.schema import TaskProps, TaskType, TaskStatus, TaskStart
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_resolve_error,
    _task6_note,
    _parse_day,
    _day_to_timestamp,
    _resolve_tag_ids,
)


def cmd_new(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Create a new task with optional container, schedule, notes, and tags."""
    title = args.title.strip()
    if not title:
        print("Task title cannot be empty.", file=sys.stderr)
        return

    now_ts = time.time()
    props = asdict(TaskProps())
    props.update(
        {
            "tt": title,
            "tp": TaskType.TODO,
            "ss": TaskStatus.INCOMPLETE,
            "st": TaskStart.INBOX,
            "tr": False,
            "cd": now_ts,
            "md": now_ts,
            "nt": _task6_note(args.notes) if args.notes else None,
            "xx": {"_t": "oo", "sn": {}},
            "rmd": None,
            "rp": None,
        }
    )

    anchor = None
    anchor_id = args.before_id if args.before_id else args.after_id
    if anchor_id:
        anchor, err, ambiguous = store.resolve_task_identifier(anchor_id)
        if not anchor:
            fmt_resolve_error(err, ambiguous, store)
            return

    in_target = (args.in_target or "inbox").strip()
    if in_target.lower() != "inbox":
        project, _perr, _pamb = store.resolve_mark_identifier(in_target)
        area, _aerr, _aamb = store.resolve_area_identifier(in_target)

        project_uuid = project.uuid if project and project.is_project else None
        area_uuid = area.uuid if area else None

        if project_uuid and area_uuid:
            print(
                f"Ambiguous --in target '{in_target}' (matches project and area).",
                file=sys.stderr,
            )
            return
        if project and not project.is_project:
            print(
                "--in target must be inbox, a project ID, or an area ID.",
                file=sys.stderr,
            )
            return
        if project_uuid:
            props["pr"] = [project_uuid]
            props["st"] = TaskStart.ANYTIME
        elif area_uuid:
            props["ar"] = [area_uuid]
            props["st"] = TaskStart.ANYTIME
        else:
            print(f"Container not found: {in_target}", file=sys.stderr)
            return

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
            # Observed cloud state often models future specific dates as
            # st=Someday with sr/tir pinned to the same day.
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

    deadline_date = getattr(args, "deadline_date", None)
    if deadline_date:
        try:
            deadline_day = _parse_day(deadline_date, "--deadline")
        except ValueError as e:
            print(str(e), file=sys.stderr)
            return
        assert deadline_day is not None
        props["dd"] = _day_to_timestamp(deadline_day)

    def _is_today_from_props(task_props: dict) -> bool:
        if task_props.get("st") != TaskStart.ANYTIME:
            return False
        sr = task_props.get("sr")
        if sr is None:
            return False
        today_ts_local = _day_to_timestamp(
            datetime.now(tz=timezone.utc).replace(
                hour=0, minute=0, second=0, microsecond=0
            )
        )
        return int(sr) <= today_ts_local

    def _task_bucket(task: Task) -> tuple:
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

    def _props_bucket(task_props: dict) -> tuple:
        project_uuid = None
        if task_props.get("pr"):
            project_uuid = task_props["pr"][0]
        if project_uuid:
            return ("task-project", project_uuid, "")

        area_uuid = None
        if task_props.get("ar"):
            area_uuid = task_props["ar"][0]
        if area_uuid:
            return ("task-area", area_uuid, task_props.get("st", TaskStart.INBOX))

        return ("task-root", task_props.get("st", TaskStart.INBOX))

    def _plan_ix_insert(
        ordered: list[Task],
        insert_at: int,
    ) -> tuple[int, list[tuple[str, int, str]]]:
        prev_ix = ordered[insert_at - 1].index if insert_at > 0 else None
        next_ix = ordered[insert_at].index if insert_at < len(ordered) else None
        updates: list[tuple[str, int, str]] = []

        if prev_ix is None and next_ix is None:
            return 0, updates
        if prev_ix is None:
            assert next_ix is not None
            return next_ix - 1, updates
        if next_ix is None:
            return prev_ix + 1, updates
        if prev_ix + 1 < next_ix:
            return (prev_ix + next_ix) // 2, updates

        stride = 1024
        new_index = stride
        ordered_with_new = ordered[:insert_at] + [None] + ordered[insert_at:]
        for idx, entry in enumerate(ordered_with_new, start=1):
            target_ix = idx * stride
            if entry is None:
                new_index = target_ix
                continue
            if entry.index != target_ix:
                updates.append((entry.uuid, target_ix, entry.entity))
        return new_index, updates

    def _today_sort_key(task: Task) -> tuple[int, int, int]:
        tir = task.today_index_reference or 0
        return (-tir, task.today_index, -task.index)

    today_ts = _day_to_timestamp(
        datetime.now(tz=timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    )
    new_is_today = _is_today_from_props(props)
    anchor_is_today = bool(
        anchor
        and anchor.start == TaskStart.ANYTIME
        and (anchor.is_today or anchor.evening)
    )
    target_bucket = _props_bucket(props)

    if anchor and not anchor_is_today and _task_bucket(anchor) != target_bucket:
        print(
            "Cannot place new task relative to an item in a different container/list.",
            file=sys.stderr,
        )
        return

    index_updates: list[tuple[str, int, str]] = []

    # Structural ordering (ix): always choose explicit relative placement when
    # possible; otherwise default to the top of the target list.
    siblings = [
        t
        for t in store._tasks.values()
        if not t.trashed
        and t.status == TaskStatus.INCOMPLETE
        and _task_bucket(t) == target_bucket
    ]
    siblings.sort(key=lambda t: (t.index, t.uuid))

    structural_insert_at = 0
    if anchor and _task_bucket(anchor) == target_bucket:
        anchor_pos = next(
            (i for i, t in enumerate(siblings) if t.uuid == anchor.uuid), None
        )
        if anchor_pos is None:
            print("Anchor not found in target list.", file=sys.stderr)
            return
        structural_insert_at = anchor_pos if args.before_id else anchor_pos + 1

    structural_ix, structural_updates = _plan_ix_insert(siblings, structural_insert_at)
    props["ix"] = structural_ix
    index_updates.extend(structural_updates)

    # Today ordering (ti/tir): if task lands in Today, place it relative to the
    # provided anchor when compatible, else default to top of its section.
    if new_is_today:
        section_evening = 1 if props.get("sb") else 0
        if anchor_is_today and anchor is not None:
            section_evening = 1 if anchor.evening else 0
            props["sb"] = section_evening

        today_siblings = [
            t
            for t in store._tasks.values()
            if not t.trashed
            and t.status == TaskStatus.INCOMPLETE
            and t.start == TaskStart.ANYTIME
            and (t.is_today or t.evening)
            and (1 if t.evening else 0) == section_evening
        ]
        today_siblings.sort(key=_today_sort_key)

        today_insert_at = 0
        if (
            anchor_is_today
            and anchor is not None
            and (1 if anchor.evening else 0) == section_evening
        ):
            anchor_today_pos = next(
                (i for i, t in enumerate(today_siblings) if t.uuid == anchor.uuid),
                None,
            )
            if anchor_today_pos is not None:
                today_insert_at = (
                    anchor_today_pos if args.before_id else anchor_today_pos + 1
                )

        prev_today = (
            today_siblings[today_insert_at - 1] if today_insert_at > 0 else None
        )
        next_today = (
            today_siblings[today_insert_at]
            if today_insert_at < len(today_siblings)
            else None
        )
        if next_today is not None:
            next_tir = next_today.today_index_reference or today_ts
            props["tir"] = next_tir
            props["ti"] = next_today.today_index - 1
        elif prev_today is not None:
            prev_tir = prev_today.today_index_reference or today_ts
            props["tir"] = prev_tir
            props["ti"] = prev_today.today_index + 1
        else:
            props["tir"] = today_ts
            props["ti"] = 0

    new_uuid = random_task_id()
    try:
        if anchor:
            changes = {new_uuid: {"t": 0, "e": "Task6", "p": props}}
            for task_uuid, task_index, task_entity in index_updates:
                changes[task_uuid] = {
                    "e": task_entity,
                    "p": {"ix": task_index, "md": now_ts},
                }
            client.commit(changes)
        else:
            client.create_task(new_uuid, props, entity="Task6")
    except Exception as e:
        print(f"Failed to create task: {e}", file=sys.stderr)
        return

    print(colored(f"{ICONS.done} Created", GREEN), f"{title}  {colored(new_uuid, DIM)}")


def register(subparsers) -> dict[str, CommandHandler]:
    new_parser = subparsers.add_parser("new", help="Create a new task")
    new_parser.add_argument("title", help="Task title")
    new_parser.add_argument(
        "--in",
        dest="in_target",
        default="inbox",
        help="Container: inbox (default), project UUID/prefix, or area UUID/prefix",
    )
    new_parser.add_argument(
        "--when",
        help="Schedule: anytime, someday, today, or YYYY-MM-DD",
    )
    new_position_group = new_parser.add_mutually_exclusive_group()
    new_position_group.add_argument(
        "--before",
        dest="before_id",
        help="Place new task before this task/project/heading UUID/prefix",
    )
    new_position_group.add_argument(
        "--after",
        dest="after_id",
        help="Place new task after this task/project/heading UUID/prefix",
    )
    new_parser.add_argument(
        "--notes",
        default="",
        help="Task notes",
    )
    new_parser.add_argument(
        "--tags",
        help="Comma-separated tags (titles or UUID prefixes)",
    )
    new_parser.add_argument(
        "--deadline",
        dest="deadline_date",
        help="Deadline date (YYYY-MM-DD)",
    )

    return {"new": cmd_new}
