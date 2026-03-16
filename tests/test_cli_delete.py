import argparse
import io
import unittest
from contextlib import redirect_stderr, redirect_stdout
from typing import Any, cast

import cli
from things_cloud.store import ThingsStore


def _delete_state() -> dict[str, dict]:
    return {
        "task-todo-000000": {
            "e": "Task6",
            "p": {"tt": "Todo", "tp": 0, "ss": 0, "st": 1, "ix": 1},
        },
        "project-00000000": {
            "e": "Task6",
            "p": {"tt": "Project", "tp": 1, "ss": 0, "st": 1, "ix": 2},
        },
        "heading-00000000": {
            "e": "Task6",
            "p": {
                "tt": "Heading",
                "tp": 2,
                "ss": 0,
                "st": 1,
                "pr": ["project-00000000"],
                "ix": 3,
            },
        },
        "area-0000000000": {
            "e": "Area3",
            "p": {"tt": "Area", "ix": 4},
        },
        "dead-task-000000": {
            "e": "Task6",
            "p": {"tt": "Dead", "tp": 0, "ss": 0, "st": 1, "ix": 5, "tr": True},
        },
        "abc-task-0000000": {
            "e": "Task6",
            "p": {"tt": "AbcTask", "tp": 0, "ss": 0, "st": 1, "ix": 6},
        },
        "abc-area-0000000": {
            "e": "Area3",
            "p": {"tt": "AbcArea", "ix": 7},
        },
    }


class _FakeClient:
    def __init__(self) -> None:
        self.calls: list[list[dict]] = []

    def delete_items(self, updates: list[dict]) -> int:
        self.calls.append(updates)
        return 1


class CmdDeleteTests(unittest.TestCase):
    def setUp(self) -> None:
        self.store = ThingsStore(_delete_state())
        self.client = _FakeClient()

    def test_delete_mixed_entities_in_single_commit(self) -> None:
        args = argparse.Namespace(
            item_ids=["task-todo", "project-0000", "heading-0000", "area-0000"],
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_delete(self.store, args, cast(Any, self.client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(self.client.calls), 1)
        updates = self.client.calls[0]
        self.assertEqual(len(updates), 4)
        entities = {item["entity"] for item in updates}
        self.assertIn("Task6", entities)
        self.assertIn("Area3", entities)

    def test_delete_rejects_cross_type_prefix_collision(self) -> None:
        args = argparse.Namespace(item_ids=["abc-"])

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_delete(self.store, args, cast(Any, self.client))

        self.assertIn("matches task and area", err.getvalue())
        self.assertEqual(len(self.client.calls), 0)

    def test_delete_skips_already_deleted_task(self) -> None:
        args = argparse.Namespace(item_ids=["dead-task"])

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_delete(self.store, args, cast(Any, self.client))

        self.assertIn("already deleted", err.getvalue())
        self.assertEqual(len(self.client.calls), 0)


if __name__ == "__main__":
    unittest.main()
