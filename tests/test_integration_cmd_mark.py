from __future__ import annotations

from tests.mutating_fixtures import checklist, store, task
from tests.mutating_http_helpers import (
    assert_commit_payloads,
    assert_no_commits,
    p,
    run_cli_mutating_http,
)


NOW = 1_700_000_111.0
TASK_A = "A7h5eCi24RvAWKC3Hv3muf"
TASK_B = "KGvAPpMrzHAKMdgMiERP1V"
TASK_C = "By8mN2qRk5Wv7Xc9Dt3HpL"
CHECK_A = "MpkEei6ybkFS2n6SXvwfLf"
CHECK_B = "JFdhhhp37fpryAKu8UXwzK"
TPL_A = "Cv9nP3sTk6Xw8Yd4Eu5JqM"
TPL_B = "Dv1oQ4uVl7Yz9Ze5Fw6KrN"


def test_done_single_payload() -> None:
    test_store = store(task(TASK_A, "Alpha"))
    result = run_cli_mutating_http(
        f"mark {TASK_A} --done",
        test_store,
        extra_patches=[
            p("things_cloud.cli.cmd_mark.time.time", return_value=NOW),
            p("things_cloud.client.time.time", return_value=NOW),
        ],
    )
    assert_commit_payloads(
        result,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"ss": 3, "sp": NOW, "md": NOW}}},
    )


def test_done_multi_payload() -> None:
    test_store = store(task(TASK_A, "Alpha"), task(TASK_B, "Beta"))
    result = run_cli_mutating_http(
        f"mark {TASK_A} {TASK_B} --done",
        test_store,
        extra_patches=[
            p("things_cloud.cli.cmd_mark.time.time", return_value=NOW),
            p("things_cloud.client.time.time", return_value=NOW),
        ],
    )
    assert_commit_payloads(
        result,
        {
            TASK_A: {"t": 1, "e": "Task6", "p": {"ss": 3, "sp": NOW, "md": NOW}},
            TASK_B: {"t": 1, "e": "Task6", "p": {"ss": 3, "sp": NOW, "md": NOW}},
        },
    )


def test_incomplete_payload() -> None:
    test_store = store(task(TASK_A, "Alpha", ss=3))
    result = run_cli_mutating_http(
        f"mark {TASK_A} --incomplete",
        test_store,
        extra_patches=[
            p("things_cloud.cli.cmd_mark.time.time", return_value=NOW),
            p("things_cloud.client.time.time", return_value=NOW),
        ],
    )
    assert_commit_payloads(
        result,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"ss": 0, "sp": None, "md": NOW}}},
    )


def test_canceled_payload() -> None:
    test_store = store(task(TASK_A, "Alpha"))
    result = run_cli_mutating_http(
        f"mark {TASK_A} --canceled",
        test_store,
        extra_patches=[
            p("things_cloud.cli.cmd_mark.time.time", return_value=NOW),
            p("things_cloud.client.time.time", return_value=NOW),
        ],
    )
    assert_commit_payloads(
        result,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"ss": 2, "sp": NOW, "md": NOW}}},
    )


def test_checklist_check_uncheck_cancel_payloads() -> None:
    test_store = store(
        task(TASK_A, "Task with checklist"),
        checklist(CHECK_A, TASK_A, "One", ix=1),
        checklist(CHECK_B, TASK_A, "Two", ix=2),
    )

    checked = run_cli_mutating_http(
        f"mark {TASK_A} --check {CHECK_A[:6]},{CHECK_B[:6]}",
        test_store,
        extra_patches=[p("things_cloud.cli.cmd_mark.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        checked,
        {
            CHECK_A: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"ss": 3, "sp": NOW, "md": NOW},
            },
            CHECK_B: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"ss": 3, "sp": NOW, "md": NOW},
            },
        },
    )

    unchecked = run_cli_mutating_http(
        f"mark {TASK_A} --uncheck {CHECK_A[:6]}",
        test_store,
        extra_patches=[p("things_cloud.cli.cmd_mark.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        unchecked,
        {
            CHECK_A: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"ss": 0, "sp": None, "md": NOW},
            }
        },
    )

    canceled = run_cli_mutating_http(
        f"mark {TASK_A} --check-cancel {CHECK_B[:6]}",
        test_store,
        extra_patches=[p("things_cloud.cli.cmd_mark.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        canceled,
        {
            CHECK_B: {
                "t": 1,
                "e": "ChecklistItem3",
                "p": {"ss": 2, "sp": NOW, "md": NOW},
            }
        },
    )


def test_checklist_flags_require_one_task_id() -> None:
    test_store = store(task(TASK_A, "A"), task(TASK_B, "B"))
    result = run_cli_mutating_http(
        f"mark {TASK_A} {TASK_B} --check abcd",
        test_store,
    )
    assert_no_commits(result)
    assert (
        result.stderr
        == "Checklist flags (--check, --uncheck, --check-cancel) require exactly one task ID.\n"
    )


def test_mark_done_skips_already_done_task() -> None:
    test_store = store(task(TASK_A, "Done task", ss=3))
    result = run_cli_mutating_http(f"mark {TASK_A} --done", test_store)
    assert_no_commits(result)
    assert result.stderr == "Task is already completed. (Done task)\n"


def test_checklist_requires_existing_items() -> None:
    result = run_cli_mutating_http(
        f"mark {TASK_A} --check {CHECK_A[:6]}",
        store(task(TASK_A, "No checklist")),
    )
    assert_no_commits(result)
    assert result.stderr == "Task has no checklist items: No checklist\n"


def test_checklist_rejects_unknown_item_prefix() -> None:
    test_store = store(
        task(TASK_A, "Task with checklist"),
        checklist(CHECK_A, TASK_A, "One", ix=1),
    )
    result = run_cli_mutating_http(
        f"mark {TASK_A} --check zzzzzz",
        test_store,
    )
    assert_no_commits(result)
    assert result.stderr == "Checklist item not found: 'zzzzzz'\n"


def test_checklist_rejects_ambiguous_item_prefix() -> None:
    check_one = "ABCD1234efgh5678JKLMno"
    check_two = "ABCD1234pqrs9123TUVWxy"
    test_store = store(
        task(TASK_A, "Task with checklist"),
        checklist(check_one, TASK_A, "One", ix=1),
        checklist(check_two, TASK_A, "Two", ix=2),
    )
    result = run_cli_mutating_http(
        f"mark {TASK_A} --check ABCD1234",
        test_store,
    )
    assert_no_commits(result)
    assert result.stderr == "Ambiguous checklist item prefix: 'ABCD1234'\n"


def test_recurring_done_rejects_template_tasks() -> None:
    test_store = store(task(TASK_A, "Recurring template", rr={"tp": 0}))
    result = run_cli_mutating_http(f"mark {TASK_A} --done", test_store)
    assert_no_commits(result)
    assert (
        result.stderr
        == "Recurring template tasks are blocked for done (template progression bookkeeping is not implemented). (Recurring template)\n"
    )


def test_recurring_done_rejects_multiple_template_refs() -> None:
    test_store = store(task(TASK_A, "Recurring instance", rt=[TPL_A, TPL_B]))
    result = run_cli_mutating_http(f"mark {TASK_A} --done", test_store)
    assert_no_commits(result)
    assert (
        result.stderr
        == "Recurring instance has 2 template references; expected exactly 1. (Recurring instance)\n"
    )


def test_recurring_done_rejects_missing_template() -> None:
    test_store = store(task(TASK_A, "Recurring instance", rt=[TPL_A]))
    result = run_cli_mutating_http(f"mark {TASK_A} --done", test_store)
    assert_no_commits(result)
    assert (
        result.stderr
        == f"Recurring instance template {TPL_A} is missing from current state. (Recurring instance)\n"
    )


def test_recurring_done_rejects_after_completion_template() -> None:
    test_store = store(
        task(TPL_A, "Template", rr={"tp": 1}),
        task(TASK_A, "Recurring instance", rt=[TPL_A]),
    )
    result = run_cli_mutating_http(f"mark {TASK_A} --done", test_store)
    assert_no_commits(result)
    assert (
        result.stderr
        == "Recurring 'after completion' templates (rr.tp=1) are blocked: completion requires coupled template writes (acrd/tir) not implemented yet. (Recurring instance)\n"
    )


def test_recurring_done_allows_fixed_schedule_instance() -> None:
    test_store = store(
        task(TPL_A, "Template", rr={"tp": 0}),
        task(TASK_A, "Recurring instance", rt=[TPL_A]),
    )
    result = run_cli_mutating_http(
        f"mark {TASK_A} --done",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"ss": 3, "sp": NOW, "md": NOW}}},
    )


def test_mark_done_handles_commit_failure() -> None:
    result = run_cli_mutating_http(
        f"mark {TASK_A} --done",
        store(task(TASK_A, "Alpha")),
        extra_patches=[
            p(
                "things_cloud.client.ThingsCloudClient.set_task_statuses",
                side_effect=RuntimeError("boom"),
            ),
        ],
    )
    assert_no_commits(result)
    assert result.stderr == "Failed to mark items done: boom\n"


def test_mark_checklist_handles_commit_failure() -> None:
    result = run_cli_mutating_http(
        f"mark {TASK_A} --check {CHECK_A[:6]}",
        store(task(TASK_A, "Task"), checklist(CHECK_A, TASK_A, "Item")),
        extra_patches=[
            p(
                "things_cloud.client.ThingsCloudClient.commit",
                side_effect=RuntimeError("boom"),
            ),
        ],
    )
    assert_no_commits(result)
    assert result.stderr == "Failed to mark checklist items: boom\n"
