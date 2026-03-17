from __future__ import annotations

from things_cloud.cli.common import _task6_note
from tests.mutating_fixtures import area, project, store, task
from tests.mutating_http_helpers import (
    assert_commit_payloads,
    assert_no_commits,
    p,
    run_cli_mutating_http,
)


NOW = 1_700_000_222.0
TASK_UUID = "A7h5eCi24RvAWKC3Hv3muf"
TASK_UUID2 = "3H9jsMx3kYMrQ4M7DReSRn"
PROJECT_UUID = "KGvAPpMrzHAKMdgMiERP1V"
AREA_UUID = "MpkEei6ybkFS2n6SXvwfLf"


def test_title_notes_and_clear_notes_payload() -> None:
    test_store = store(task(TASK_UUID, "Old title"))

    title = run_cli_mutating_http(
        f'edit {TASK_UUID} --title "New title"',
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        title,
        {TASK_UUID: {"t": 1, "e": "Task6", "p": {"tt": "New title", "md": NOW}}},
    )

    notes = run_cli_mutating_http(
        f'edit {TASK_UUID} --notes "new notes"',
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        notes,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"nt": _task6_note("new notes"), "md": NOW},
            }
        },
    )

    clear = run_cli_mutating_http(
        f"edit {TASK_UUID} --notes ''",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        clear,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"nt": {"_t": "tx", "t": 1, "ch": 0, "v": ""}, "md": NOW},
            }
        },
    )


def test_move_targets_payload() -> None:
    test_store = store(
        task(TASK_UUID, "Movable", st=0),
        project(PROJECT_UUID, "Roadmap"),
        area(AREA_UUID, "Work"),
    )

    inbox = run_cli_mutating_http(
        f"edit {TASK_UUID} --move inbox",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        inbox,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {
                    "pr": [],
                    "ar": [],
                    "agr": [],
                    "st": 0,
                    "sr": None,
                    "tir": None,
                    "sb": 0,
                    "md": NOW,
                },
            }
        },
    )

    clear = run_cli_mutating_http(
        f"edit {TASK_UUID} --move clear",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        clear,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"pr": [], "ar": [], "agr": [], "st": 1, "md": NOW},
            }
        },
    )

    project_move = run_cli_mutating_http(
        f"edit {TASK_UUID} --move {PROJECT_UUID}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        project_move,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"pr": [PROJECT_UUID], "ar": [], "agr": [], "st": 1, "md": NOW},
            }
        },
    )

    area_move = run_cli_mutating_http(
        f"edit {TASK_UUID} --move {AREA_UUID}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        area_move,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"ar": [AREA_UUID], "pr": [], "agr": [], "st": 1, "md": NOW},
            }
        },
    )


def test_multi_id_move_sends_single_commit() -> None:
    test_store = store(
        task(TASK_UUID, "Task One", st=0),
        task(TASK_UUID2, "Task Two", st=0),
        project(PROJECT_UUID, "Roadmap"),
    )
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} {TASK_UUID2} --move {PROJECT_UUID}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {
            TASK_UUID: {
                "t": 1,
                "e": "Task6",
                "p": {"pr": [PROJECT_UUID], "ar": [], "agr": [], "st": 1, "md": NOW},
            },
            TASK_UUID2: {
                "t": 1,
                "e": "Task6",
                "p": {"pr": [PROJECT_UUID], "ar": [], "agr": [], "st": 1, "md": NOW},
            },
        },
    )


def test_multi_id_title_is_rejected() -> None:
    result = run_cli_mutating_http(
        f'edit {TASK_UUID} {TASK_UUID2} --title "New"',
        store(task(TASK_UUID, "A"), task(TASK_UUID2, "B")),
    )
    assert_no_commits(result)
    assert result.stderr == "--title requires a single task ID.\n"


def test_multi_id_notes_is_rejected() -> None:
    result = run_cli_mutating_http(
        f'edit {TASK_UUID} {TASK_UUID2} --notes "note"',
        store(task(TASK_UUID, "A"), task(TASK_UUID2, "B")),
    )
    assert_no_commits(result)
    assert result.stderr == "--notes requires a single task ID.\n"


def test_no_changes_requested_is_rejected() -> None:
    result = run_cli_mutating_http(f"edit {TASK_UUID}", store(task(TASK_UUID, "A")))
    assert_no_commits(result)
    assert result.stderr == "No edit changes requested.\n"


def test_edit_project_is_rejected() -> None:
    result = run_cli_mutating_http(
        f"edit {PROJECT_UUID} --title 'New'",
        store(project(PROJECT_UUID, "Roadmap")),
    )
    assert_no_commits(result)
    assert result.stderr == "Use 'projects edit' to edit a project.\n"


def test_move_target_must_be_project_or_area() -> None:
    test_store = store(
        task(TASK_UUID, "Movable"),
        task(PROJECT_UUID, "Not a project", tp=0),
    )
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --move {PROJECT_UUID}",
        test_store,
    )
    assert_no_commits(result)
    assert (
        result.stderr
        == "--move target must be Inbox, clear, a project ID, or an area ID.\n"
    )


def test_move_target_ambiguous_between_project_and_area() -> None:
    ambiguous_project = "ABCD1234efgh5678JKLMno"
    ambiguous_area = "ABCD1234pqrs9123TUVWxy"
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --move ABCD1234",
        store(
            task(TASK_UUID, "Movable"),
            project(ambiguous_project, "Project match"),
            area(ambiguous_area, "Area match"),
        ),
    )
    assert_no_commits(result)
    assert (
        result.stderr
        == "Ambiguous --move target 'ABCD1234' (matches project and area).\n"
    )
