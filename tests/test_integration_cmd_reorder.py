from __future__ import annotations

from tests.mutating_fixtures import store, task, today_ts
from tests.mutating_http_helpers import (
    assert_commit_payloads,
    assert_no_commits,
    p,
    run_cli_mutating_http,
)


NOW = 1_700_000_444.0
TASK_A = "A7h5eCi24RvAWKC3Hv3muf"
TASK_B = "KGvAPpMrzHAKMdgMiERP1V"
TASK_C = "MpkEei6ybkFS2n6SXvwfLf"


def test_before_after_inbox_payloads() -> None:
    test_store = store(
        task(TASK_A, "A", ix=1024),
        task(TASK_B, "B", ix=2048),
        task(TASK_C, "C", ix=3072),
    )

    before = run_cli_mutating_http(
        f"reorder {TASK_C} --before {TASK_B}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        before,
        {TASK_C: {"t": 1, "e": "Task6", "p": {"ix": 1536, "md": NOW}}},
    )

    after = run_cli_mutating_http(
        f"reorder {TASK_A} --after {TASK_B}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        after,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"ix": 2560, "md": NOW}}},
    )


def test_rebalance_payloads_and_ancestors() -> None:
    test_store = store(
        task(TASK_A, "A", ix=1024),
        task(TASK_B, "B", ix=1025),
        task(TASK_C, "C", ix=1026),
    )
    result = run_cli_mutating_http(
        f"reorder {TASK_C} --after {TASK_A}",
        test_store,
        initial_head_index=50,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )

    assert [c.ancestor_index for c in result.commits] == [50, 51]
    assert_commit_payloads(
        result,
        {TASK_C: {"t": 1, "e": "Task6", "p": {"ix": 2048, "md": NOW}}},
        {TASK_B: {"t": 1, "e": "Task6", "p": {"ix": 3072, "md": NOW}}},
    )


def test_today_payload() -> None:
    today = today_ts()
    test_store = store(
        task(TASK_A, "A", st=1, sr=today, tir=today, ti=10, ix=100),
        task(TASK_B, "B", st=1, sr=today, tir=today, ti=20, ix=200),
    )
    result = run_cli_mutating_http(
        f"reorder {TASK_A} --after {TASK_B}",
        test_store,
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TASK_A: {"t": 1, "e": "Task6", "p": {"tir": today, "ti": 21, "md": NOW}}},
    )


def test_cannot_reorder_relative_to_self() -> None:
    test_store = store(task(TASK_A, "A", ix=100))
    result = run_cli_mutating_http(f"reorder {TASK_A} --before {TASK_A}", test_store)
    assert_no_commits(result)
    assert result.stderr == "Cannot reorder an item relative to itself.\n"


def test_cannot_reorder_across_different_containers() -> None:
    test_store = store(
        task(TASK_A, "Inbox task", st=0, ix=100),
        task(TASK_B, "Project task", st=1, pr=[TASK_C], ix=200),
        task(TASK_C, "Project", tp=1, st=1, ix=10),
    )
    result = run_cli_mutating_http(
        f"reorder {TASK_A} --before {TASK_B}",
        test_store,
    )
    assert_no_commits(result)
    assert result.stderr == "Cannot reorder across different containers/lists.\n"


def test_anchor_not_in_selected_list_is_rejected() -> None:
    test_store = store(
        task(TASK_A, "Open", st=0, ss=0, ix=100),
        task(TASK_B, "Done anchor", st=0, ss=3, ix=200),
        task(TASK_C, "Other open", st=0, ss=0, ix=300),
    )
    result = run_cli_mutating_http(
        f"reorder {TASK_A} --after {TASK_B}",
        test_store,
    )
    assert_no_commits(result)
    assert result.stderr == "Cannot reorder item in the selected list.\n"
