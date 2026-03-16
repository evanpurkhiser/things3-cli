"""Single project detail view command."""

import argparse
import sys

from things_cloud.store import ThingsStore
from things_cloud.cli.common import (
    BOLD,
    GREEN,
    DIM,
    ICONS,
    CommandHandler,
    colored,
    fmt_task_line,
    fmt_project_line,
    fmt_deadline,
    print_task_with_note,
    _adapt_store_command,
)


def cmd_project(store: ThingsStore, args: argparse.Namespace) -> None:
    """Show all tasks in a specific project, grouped by heading."""
    detailed = args.detailed
    task, err, ambiguous = store.resolve_mark_identifier(args.project_id)
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
    if not task.is_project:
        print(f"Not a project: {task.title}", file=sys.stderr)
        return

    project = task

    # Collect incomplete, non-trashed child items (tasks + headings)
    children = [
        t
        for t in store.tasks(status=None, trashed=False)
        if store.effective_project_uuid(t) == project.uuid
    ]

    # Also collect headings (store.tasks() always excludes them)
    headings = {
        t.uuid: t
        for t in store._tasks.values()
        if t.is_heading and not t.trashed and t.project == project.uuid
    }

    # Split children by heading
    ungrouped = []
    by_heading: dict[str, list] = {}
    for t in children:
        heading_uuid = t.action_group
        if heading_uuid and heading_uuid in headings:
            by_heading.setdefault(heading_uuid, []).append(t)
        else:
            ungrouped.append(t)

    # Sort headings by index, tasks within each group by index
    sorted_heading_uuids = sorted(
        by_heading.keys(),
        key=lambda u: headings[u].index,
    )
    ungrouped.sort(key=lambda t: t.index)
    for tasks in by_heading.values():
        tasks.sort(key=lambda t: t.index)

    total = len(children)
    progress = store.project_progress(project.uuid)
    done_count = progress.done

    # Header
    tags = ""
    if project.tags:
        tag_names = [store.resolve_tag_title(t) for t in project.tags]
        tags = colored(" [" + ", ".join(tag_names) + "]", DIM)
    print(
        colored(
            f"{ICONS.project} {project.title}  ({done_count}/{done_count + total})",
            BOLD + GREEN,
        )
        + fmt_deadline(project.deadline)
        + tags
    )
    if project.notes:
        note_lines = project.notes.splitlines()
        for note_line in note_lines[:-1]:
            print(colored("  " + "│", DIM) + " " + colored(note_line, DIM))
        print(colored("  " + "└", DIM) + " " + colored(note_lines[-1], DIM))

    all_uuids = [project.uuid] + [t.uuid for t in children]
    id_prefix_len = store.unique_prefix_length(all_uuids)

    if not children:
        print(colored("  No tasks.", DIM))
        return

    # Ungrouped tasks first
    if ungrouped:
        print()
        for t in ungrouped:
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

    # Then heading groups
    for heading_uuid in sorted_heading_uuids:
        heading = headings[heading_uuid]
        heading_tasks = by_heading[heading_uuid]
        print()
        print(colored(f"  {heading.title}", BOLD))
        for t in heading_tasks:
            line = fmt_task_line(
                t, store, show_today_markers=True, id_prefix_len=id_prefix_len
            )
            print_task_with_note(
                line,
                t,
                "    ",
                show_today_markers=True,
                id_prefix_len=id_prefix_len,
                detailed=detailed,
            )


def register(subparsers, parents: dict) -> dict[str, CommandHandler]:
    detailed_parent = parents["detailed"]
    project_parser = subparsers.add_parser(
        "project", help="Show all tasks in a project", parents=[detailed_parent]
    )
    project_parser.add_argument(
        "project_id",
        help="Project UUID (or unique UUID prefix)",
    )
    return {"project": _adapt_store_command(cmd_project)}
