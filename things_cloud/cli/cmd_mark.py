"""Mark task/project/checklist-item command."""

import argparse
import sys
import time

from things_cloud.client import ThingsCloudClient
from things_cloud.store import ThingsStore, Task, ChecklistItem
from things_cloud.schema import ChecklistStatus
from things_cloud.cli.common import (
    GREEN,
    DIM,
    ICONS,
    colored,
    fmt_task_line,
    fmt_project_line,
    RECURRENCE_FIXED_SCHEDULE,
    RECURRENCE_AFTER_COMPLETION,
)


def _validate_recurring_done(task: Task, store: ThingsStore) -> tuple[bool, str]:
    """Validate whether recurring completion can be done safely.

    Historical cloud data shows two distinct recurring completion patterns:
    - Fixed schedule templates (rr.tp=0): instance completion is typically only
      the instance mutation (`ss=3, sp=now, md=now`).
    - After completion templates (rr.tp=1): completion often couples template
      writes (`acrd`, `tir`, and sometimes `rr.ia`) in the same commit item.

    To fail closed, we only allow recurring *instances* linked to templates with
    rr.tp=0. Everything else is blocked with an explicit message.
    """
    if task.is_recurrence_template:
        return (
            False,
            "Recurring template tasks are blocked for done (template progression bookkeeping is not implemented).",
        )

    if not task.is_recurrence_instance:
        return (
            False,
            "Recurring task shape is unsupported (expected an instance with rt set and rr unset).",
        )

    if len(task.recurrence_templates) != 1:
        return (
            False,
            f"Recurring instance has {len(task.recurrence_templates)} template references; expected exactly 1.",
        )

    template_uuid = task.recurrence_templates[0]
    template = store.get_task(template_uuid)
    if not template:
        return (
            False,
            f"Recurring instance template {template_uuid} is missing from current state.",
        )

    rr = template.recurrence_rule
    if not isinstance(rr, dict):
        return (
            False,
            "Recurring instance template has unsupported recurrence rule shape (expected dict).",
        )

    rr_type = rr.get("tp")
    if rr_type == RECURRENCE_FIXED_SCHEDULE:
        return True, ""
    if rr_type == RECURRENCE_AFTER_COMPLETION:
        return (
            False,
            "Recurring 'after completion' templates (rr.tp=1) are blocked: completion requires coupled template writes (acrd/tir) not implemented yet.",
        )

    return (
        False,
        f"Recurring template type rr.tp={rr_type!r} is unsupported for safe completion.",
    )


def _validate_mark_target(task: Task, action: str, store: ThingsStore) -> str:
    """Return an error message when *task* cannot be marked for *action*."""
    if task.entity != "Task6":
        return "Only Task6 tasks are supported by mark right now."
    if task.is_heading:
        return "Headings cannot be marked."
    if task.trashed:
        return "Task is in Trash and cannot be completed."
    if action == "done" and task.status == 3:
        return "Task is already completed."
    if action == "incomplete" and task.status == 0:
        return "Task is already incomplete/open."
    if action == "canceled" and task.status == 2:
        return "Task is already canceled."
    if action == "done" and task.is_recurring:
        ok, reason = _validate_recurring_done(task, store)
        if not ok:
            return reason
    return ""


def _resolve_checklist_items(
    task: Task, raw_ids: str
) -> tuple[list[ChecklistItem], str]:
    """Resolve comma-separated short ID prefixes against a task's checklist items.

    Returns (matched_items, error_message). Error is non-empty on any failure.
    """
    tokens = [t.strip() for t in raw_ids.split(",") if t.strip()]
    if not tokens:
        return [], "No checklist item IDs provided."

    items = task.checklist_items
    resolved: list[ChecklistItem] = []
    seen: set[str] = set()

    for token in tokens:
        matches = [item for item in items if item.uuid.startswith(token)]
        if not matches:
            return [], f"Checklist item not found: {token!r}"
        if len(matches) > 1:
            return [], f"Ambiguous checklist item prefix: {token!r}"
        item = matches[0]
        if item.uuid not in seen:
            seen.add(item.uuid)
            resolved.append(item)

    return resolved, ""


def cmd_mark(
    store: ThingsStore, args: argparse.Namespace, client: ThingsCloudClient
) -> None:
    """Mark one or more tasks/projects by UUID (or unique UUID prefix)."""
    # Checklist item marking
    checklist_raw = (
        getattr(args, "check_ids", None)
        or getattr(args, "uncheck_ids", None)
        or getattr(args, "check_cancel_ids", None)
    )
    if checklist_raw:
        if len(args.task_ids) != 1:
            print(
                "Checklist flags (--check, --uncheck, --check-cancel) require exactly one task ID.",
                file=sys.stderr,
            )
            return

        task, err, ambiguous = store.resolve_mark_identifier(args.task_ids[0])
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

        if not task.checklist_items:
            print(f"Task has no checklist items: {task.title}", file=sys.stderr)
            return

        cl_items, cl_err = _resolve_checklist_items(task, checklist_raw)
        if cl_err:
            print(cl_err, file=sys.stderr)
            return

        if args.check_ids:
            cl_action, cl_status = "checked", ChecklistStatus.COMPLETED
        elif args.uncheck_ids:
            cl_action, cl_status = "unchecked", ChecklistStatus.INCOMPLETE
        else:
            cl_action, cl_status = "canceled", ChecklistStatus.CANCELED

        now = time.time()
        stop_date = (
            now
            if cl_status in {ChecklistStatus.COMPLETED, ChecklistStatus.CANCELED}
            else None
        )
        changes = {
            item.uuid: {
                "e": "ChecklistItem3",
                "p": {"ss": cl_status, "sp": stop_date, "md": now},
            }
            for item in cl_items
        }

        try:
            client.commit(changes)
        except Exception as e:
            print(f"Failed to mark checklist items: {e}", file=sys.stderr)
            return

        label = {
            "checked": f"{ICONS.checklist_done} Checked",
            "unchecked": f"{ICONS.checklist_open} Unchecked",
            "canceled": f"{ICONS.checklist_canceled} Canceled",
        }[cl_action]
        for item in cl_items:
            print(colored(label, GREEN), f"{item.title}  {colored(item.uuid, DIM)}")
        return

    # Task/project marking
    action = "done" if args.done else "incomplete" if args.incomplete else "canceled"

    targets: list[Task] = []
    seen: set[str] = set()
    for identifier in args.task_ids:
        task, err, ambiguous = store.resolve_mark_identifier(identifier)
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
            continue
        if task.uuid in seen:
            continue
        seen.add(task.uuid)
        targets.append(task)

    updates: list[dict] = []
    successes: list[Task] = []

    for task in targets:
        validation_error = _validate_mark_target(task, action, store)
        if validation_error:
            print(f"{validation_error} ({task.title})", file=sys.stderr)
            continue

        stop_date = time.time() if action in {"done", "canceled"} else None
        updates.append(
            {
                "task_uuid": task.uuid,
                "status": 3 if action == "done" else 0 if action == "incomplete" else 2,
                "entity": task.entity,
                "stop_date": stop_date,
            }
        )
        successes.append(task)

    if not updates:
        return

    try:
        client.set_task_statuses(updates)
    except Exception as e:
        print(f"Failed to mark items {action}: {e}", file=sys.stderr)
        return

    label = {
        "done": f"{ICONS.done} Done",
        "incomplete": f"{ICONS.incomplete} Incomplete",
        "canceled": f"{ICONS.canceled} Canceled",
    }[action]
    for task in successes:
        print(colored(label, GREEN), f"{task.title}  {colored(task.uuid, DIM)}")


def register(subparsers, parents: dict) -> dict:
    mark_parser = subparsers.add_parser(
        "mark", help="Mark a task done, incomplete, or canceled"
    )
    mark_parser.add_argument(
        "task_ids",
        nargs="+",
        help="Task/Project UUID(s) (or unique UUID prefixes)",
    )
    mark_group = mark_parser.add_mutually_exclusive_group(required=True)
    mark_group.add_argument(
        "--done",
        action="store_true",
        help="Set status to completed",
    )
    mark_group.add_argument(
        "--incomplete",
        action="store_true",
        help="Set status to open/incomplete",
    )
    mark_group.add_argument(
        "--canceled",
        action="store_true",
        help="Set status to canceled",
    )
    mark_group.add_argument(
        "--check",
        dest="check_ids",
        metavar="IDS",
        help="Mark checklist items done (comma-separated short IDs, single task only)",
    )
    mark_group.add_argument(
        "--uncheck",
        dest="uncheck_ids",
        metavar="IDS",
        help="Mark checklist items incomplete (comma-separated short IDs, single task only)",
    )
    mark_group.add_argument(
        "--check-cancel",
        dest="check_cancel_ids",
        metavar="IDS",
        help="Mark checklist items canceled (comma-separated short IDs, single task only)",
    )

    def _run_mark(store, args, client):
        cmd_mark(store, args, client)
        return None

    return {"mark": _run_mark}
