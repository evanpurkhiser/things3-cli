import argparse
import io
import unittest
from contextlib import redirect_stderr, redirect_stdout
from datetime import datetime, timezone
from typing import Any, cast
from unittest.mock import patch

import cli
from things_cloud.store import ThingsStore


class _FakeClient:
    def __init__(self) -> None:
        self.create_calls: list[tuple[str, dict, str]] = []
        self.commit_calls: list[dict] = []

    def create_task(self, task_uuid: str, props: dict, entity: str = "Task6") -> int:
        self.create_calls.append((task_uuid, props, entity))
        return 1

    def commit(self, changes: dict, ancestor_index: int | None = None) -> int:
        self.commit_calls.append(changes)
        return 1


def _today_ts() -> int:
    return int(
        datetime.now(tz=timezone.utc)
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )


class CmdNewTests(unittest.TestCase):
    def test_new_without_position_defaults_to_top_of_target_list(self) -> None:
        state = {
            "task-first-000": {
                "e": "Task6",
                "p": {"tt": "First", "ss": 0, "st": 0, "ix": 100},
            },
            "task-second-00": {
                "e": "Task6",
                "p": {"tt": "Second", "ss": 0, "st": 0, "ix": 200},
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            title="New task",
            in_target="inbox",
            when=None,
            notes="",
            tags=None,
            before_id=None,
            after_id=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with patch(
            "things_cloud.cli.cmd_new.random_task_id", return_value="task-new-000"
        ):
            with redirect_stdout(out), redirect_stderr(err):
                cli.cmd_new(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.create_calls), 1)
        _task_uuid, props, _entity = client.create_calls[0]
        self.assertEqual(props["ix"], 99)

    def test_new_after_inbox_anchor_uses_single_commit_with_position(self) -> None:
        state = {
            "task-anchor-0": {
                "e": "Task6",
                "p": {"tt": "Anchor", "ss": 0, "st": 0, "ix": 100},
            },
            "task-next-000": {
                "e": "Task6",
                "p": {"tt": "Next", "ss": 0, "st": 0, "ix": 200},
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            title="New task",
            in_target="inbox",
            when=None,
            notes="",
            tags=None,
            before_id=None,
            after_id="task-anchor",
        )

        out = io.StringIO()
        err = io.StringIO()
        with patch(
            "things_cloud.cli.cmd_new.random_task_id", return_value="task-new-000"
        ):
            with redirect_stdout(out), redirect_stderr(err):
                cli.cmd_new(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.create_calls), 0)
        self.assertEqual(len(client.commit_calls), 1)

        changes = client.commit_calls[0]
        self.assertIn("task-new-000", changes)
        self.assertEqual(changes["task-new-000"]["t"], 0)
        self.assertEqual(changes["task-new-000"]["p"]["ix"], 150)
        self.assertEqual(len(changes), 1)

    def test_new_after_inbox_anchor_rebalances_in_same_commit(self) -> None:
        state = {
            "task-anchor-0": {
                "e": "Task6",
                "p": {"tt": "Anchor", "ss": 0, "st": 0, "ix": 100},
            },
            "task-next-000": {
                "e": "Task6",
                "p": {"tt": "Next", "ss": 0, "st": 0, "ix": 101},
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            title="New task",
            in_target="inbox",
            when=None,
            notes="",
            tags=None,
            before_id=None,
            after_id="task-anchor",
        )

        out = io.StringIO()
        err = io.StringIO()
        with patch(
            "things_cloud.cli.cmd_new.random_task_id", return_value="task-new-000"
        ):
            with redirect_stdout(out), redirect_stderr(err):
                cli.cmd_new(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.create_calls), 0)
        self.assertEqual(len(client.commit_calls), 1)

        changes = client.commit_calls[0]
        self.assertEqual(changes["task-new-000"]["p"]["ix"], 2048)
        self.assertEqual(changes["task-anchor-0"]["p"]["ix"], 1024)
        self.assertEqual(changes["task-next-000"]["p"]["ix"], 3072)

    def test_new_after_today_anchor_sets_today_order_fields(self) -> None:
        day_ts = _today_ts()
        state = {
            "task-anchor-0": {
                "e": "Task6",
                "p": {
                    "tt": "Anchor",
                    "ss": 0,
                    "st": 1,
                    "sr": day_ts,
                    "tir": day_ts,
                    "ti": 10,
                    "sb": 1,
                    "ix": 100,
                },
            }
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            title="New task",
            in_target="inbox",
            when="today",
            notes="",
            tags=None,
            before_id=None,
            after_id="task-anchor",
        )

        out = io.StringIO()
        err = io.StringIO()
        with patch(
            "things_cloud.cli.cmd_new.random_task_id", return_value="task-new-000"
        ):
            with redirect_stdout(out), redirect_stderr(err):
                cli.cmd_new(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.commit_calls), 1)
        props = client.commit_calls[0]["task-new-000"]["p"]
        self.assertEqual(props["ti"], 11)
        self.assertEqual(props["tir"], day_ts)
        self.assertEqual(props["sb"], 1)

    def test_new_in_project_today_sets_structural_and_today_indexes(self) -> None:
        day_ts = _today_ts()
        state = {
            "project-000000": {
                "e": "Task6",
                "p": {"tt": "Project", "tp": 1, "ss": 0, "st": 1, "ix": 10},
            },
            "task-project-1": {
                "e": "Task6",
                "p": {
                    "tt": "Project task",
                    "ss": 0,
                    "st": 1,
                    "pr": ["project-000000"],
                    "ix": 300,
                },
            },
            "task-today-001": {
                "e": "Task6",
                "p": {
                    "tt": "Today task",
                    "ss": 0,
                    "st": 1,
                    "sr": day_ts,
                    "tir": day_ts,
                    "ti": 50,
                    "ix": 100,
                },
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            title="New project task",
            in_target="project-000000",
            when="today",
            notes="",
            tags=None,
            before_id=None,
            after_id=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with patch(
            "things_cloud.cli.cmd_new.random_task_id", return_value="task-new-000"
        ):
            with redirect_stdout(out), redirect_stderr(err):
                cli.cmd_new(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.create_calls), 1)
        _task_uuid, props, _entity = client.create_calls[0]
        self.assertEqual(props["ix"], 299)
        self.assertEqual(props["tir"], day_ts)
        self.assertEqual(props["ti"], 49)


if __name__ == "__main__":
    unittest.main()
