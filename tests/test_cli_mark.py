import argparse
import io
import unittest
from contextlib import redirect_stderr, redirect_stdout
from typing import Any, cast

import cli
from things_cloud.store import ThingsStore


def _mark_state() -> dict[str, dict]:
    return {
        "task-alpha-000000": {
            "e": "Task6",
            "p": {"tt": "Alpha", "ss": 0, "st": 1, "ix": 1},
        },
        "task-beta-0000000": {
            "e": "Task6",
            "p": {"tt": "Beta", "ss": 0, "st": 1, "ix": 2},
        },
    }


class _FakeClient:
    def __init__(self) -> None:
        self.calls: list[list[dict]] = []

    def set_task_statuses(self, updates: list[dict]) -> int:
        self.calls.append(updates)
        return 123


class CmdMarkTests(unittest.TestCase):
    def setUp(self) -> None:
        self.store = ThingsStore(_mark_state())
        self.client = _FakeClient()

    def test_multimark_batches_into_single_cloud_commit(self) -> None:
        args = argparse.Namespace(
            task_ids=["task-alpha", "task-beta"],
            done=True,
            incomplete=False,
            canceled=False,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_mark(self.store, args, cast(Any, self.client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(self.client.calls), 1)
        updates = self.client.calls[0]
        self.assertEqual(len(updates), 2)
        uuids = {item["task_uuid"] for item in updates}
        self.assertEqual(uuids, {"task-alpha-000000", "task-beta-0000000"})

    def test_multimark_deduplicates_repeated_identifiers(self) -> None:
        args = argparse.Namespace(
            task_ids=["task-alpha", "task-alpha-000000", "task-beta"],
            done=True,
            incomplete=False,
            canceled=False,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_mark(self.store, args, cast(Any, self.client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(self.client.calls), 1)
        updates = self.client.calls[0]
        self.assertEqual(len(updates), 2)


if __name__ == "__main__":
    unittest.main()
