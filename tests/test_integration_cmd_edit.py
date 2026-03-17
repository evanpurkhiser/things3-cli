from __future__ import annotations

from things_cloud.cli.common import _task6_note
from tests.mutating_fixtures import area, checklist, project, store, tag, task
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


def test_add_tags_payload() -> None:
    TAG1 = "WukwpDdL5Z88nX3okGMKTC"
    TAG2 = "JiqwiDaS3CAyjCmHihBDnB"
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --add-tags Work,Focus",
        store(task(TASK_UUID, "A"), tag(TAG1, "Work"), tag(TAG2, "Focus")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TASK_UUID: {"t": 1, "e": "Task6", "p": {"tg": [TAG1, TAG2], "md": NOW}}},
    )


def test_remove_tags_payload() -> None:
    TAG1 = "WukwpDdL5Z88nX3okGMKTC"
    TAG2 = "JiqwiDaS3CAyjCmHihBDnB"
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --remove-tags Work",
        store(
            task(TASK_UUID, "A", tg=[TAG1, TAG2]), tag(TAG1, "Work"), tag(TAG2, "Focus")
        ),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TASK_UUID: {"t": 1, "e": "Task6", "p": {"tg": [TAG2], "md": NOW}}},
    )


def test_add_and_remove_tags_combined() -> None:
    TAG1 = "WukwpDdL5Z88nX3okGMKTC"
    TAG2 = "JiqwiDaS3CAyjCmHihBDnB"
    TAG3 = "QHzcr8ds3Ujotkj8niP12z"
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --add-tags Focus --remove-tags Work",
        store(
            task(TASK_UUID, "A", tg=[TAG1]),
            tag(TAG1, "Work"),
            tag(TAG2, "Focus"),
            tag(TAG3, "Other"),
        ),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TASK_UUID: {"t": 1, "e": "Task6", "p": {"tg": [TAG2], "md": NOW}}},
    )


def test_add_tags_multi_id_single_commit() -> None:
    TAG1 = "WukwpDdL5Z88nX3okGMKTC"
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} {TASK_UUID2} --add-tags Work",
        store(task(TASK_UUID, "A"), task(TASK_UUID2, "B"), tag(TAG1, "Work")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {
            TASK_UUID: {"t": 1, "e": "Task6", "p": {"tg": [TAG1], "md": NOW}},
            TASK_UUID2: {"t": 1, "e": "Task6", "p": {"tg": [TAG1], "md": NOW}},
        },
    )


CHECK_A = "5uwoHPi5m5i8QJa6Rae6Cn"
CHECK_B = "CwhFwmHxjHkR7AFn9aJH9Q"
NEW_CHECK_UUID_1 = "6AdRzMSdFcjqexdTkAUAMj"
NEW_CHECK_UUID_2 = "HnP7kRw2mXsYtLvBqUeZoN"


def test_add_checklist_items_payload() -> None:
    result = run_cli_mutating_http(
        f'edit {TASK_UUID} --add-checklist "Step one" --add-checklist "Step two"',
        store(task(TASK_UUID, "A")),
        extra_patches=[
            p(
                "things_cloud.cli.cmd_edit.random_task_id",
                side_effect=[NEW_CHECK_UUID_1, NEW_CHECK_UUID_2],
            ),
            p("things_cloud.client.time.time", return_value=NOW),
        ],
    )
    assert_commit_payloads(
        result,
        {
            NEW_CHECK_UUID_1: {
                "t": 0,
                "e": "ChecklistItem3",
                "p": {
                    "tt": "Step one",
                    "ts": [TASK_UUID],
                    "ss": 0,
                    "ix": 1,
                    "cd": NOW,
                    "md": NOW,
                },
            },
            NEW_CHECK_UUID_2: {
                "t": 0,
                "e": "ChecklistItem3",
                "p": {
                    "tt": "Step two",
                    "ts": [TASK_UUID],
                    "ss": 0,
                    "ix": 2,
                    "cd": NOW,
                    "md": NOW,
                },
            },
        },
    )


def test_remove_checklist_items_payload() -> None:
    result = run_cli_mutating_http(
        f"edit {TASK_UUID} --remove-checklist {CHECK_A[:6]},{CHECK_B[:6]}",
        store(
            task(TASK_UUID, "A"),
            checklist(CHECK_A, TASK_UUID, "Step one", ix=1),
            checklist(CHECK_B, TASK_UUID, "Step two", ix=2),
        ),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {
            CHECK_A: {"t": 2, "e": "ChecklistItem3", "p": {}},
            CHECK_B: {"t": 2, "e": "ChecklistItem3", "p": {}},
        },
    )


def test_rename_checklist_items_payload() -> None:
    result = run_cli_mutating_http(
        f'edit {TASK_UUID} --rename-checklist {CHECK_A[:6]}:"New step one" --rename-checklist {CHECK_B[:6]}:"New step two"',
        store(
            task(TASK_UUID, "A"),
            checklist(CHECK_A, TASK_UUID, "Step one", ix=1),
            checklist(CHECK_B, TASK_UUID, "Step two", ix=2),
        ),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {
            CHECK_A: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"tt": "New step one", "md": NOW},
            },
            CHECK_B: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"tt": "New step two", "md": NOW},
            },
        },
    )


def test_checklist_flags_require_single_task_id() -> None:
    result = run_cli_mutating_http(
        f'edit {TASK_UUID} {TASK_UUID2} --add-checklist "Step"',
        store(task(TASK_UUID, "A"), task(TASK_UUID2, "B")),
    )
    assert_no_commits(result)
    assert (
        result.stderr
        == "--add-checklist/--remove-checklist/--rename-checklist require a single task ID.\n"
    )


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
