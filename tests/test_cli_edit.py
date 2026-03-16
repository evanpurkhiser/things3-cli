import argparse
import io
import unittest
from contextlib import redirect_stderr, redirect_stdout
from datetime import datetime, timezone
from typing import Any, cast

import cli
from things_cloud.store import ThingsStore


def _today_ts() -> int:
    return int(
        datetime.now(tz=timezone.utc)
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )


class _FakeClient:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict, str]] = []

    def update_task_fields(
        self, task_uuid: str, props: dict, entity: str = "Task6"
    ) -> int:
        self.calls.append((task_uuid, props, entity))
        return 1


class CmdEditTests(unittest.TestCase):
    def test_edit_title_updates_tt(self) -> None:
        state = {
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Original", "ss": 0, "st": 0, "ix": 1},
            }
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title="Renamed",
            move_target=None,
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        task_uuid, props, entity = client.calls[0]
        self.assertEqual(task_uuid, "task-edit-0000")
        self.assertEqual(entity, "Task6")
        self.assertEqual(props, {"tt": "Renamed"})

    def test_edit_move_inbox_clears_schedule_fields(self) -> None:
        day_ts = _today_ts()
        state = {
            "project-000000": {
                "e": "Task6",
                "p": {"tt": "Project", "tp": 1, "ss": 0, "st": 1, "ix": 10},
            },
            "heading-000000": {
                "e": "Task6",
                "p": {
                    "tt": "Heading",
                    "tp": 2,
                    "ss": 0,
                    "st": 1,
                    "pr": ["project-000000"],
                    "ix": 11,
                },
            },
            "task-edit-0000": {
                "e": "Task6",
                "p": {
                    "tt": "Task",
                    "ss": 0,
                    "st": 1,
                    "sr": day_ts,
                    "tir": day_ts,
                    "sb": 1,
                    "pr": ["project-000000"],
                    "agr": ["heading-000000"],
                    "ix": 12,
                },
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target="Inbox",
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["st"], 0)
        self.assertIsNone(props["sr"])
        self.assertIsNone(props["tir"])
        self.assertEqual(props["sb"], 0)
        self.assertEqual(props["pr"], [])
        self.assertEqual(props["ar"], [])
        self.assertEqual(props["agr"], [])

    def test_edit_move_project_from_inbox_sets_anytime(self) -> None:
        state = {
            "project-000000": {
                "e": "Task6",
                "p": {"tt": "Project", "tp": 1, "ss": 0, "st": 1, "ix": 10},
            },
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Task", "ss": 0, "st": 0, "ix": 1},
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target="project-000000",
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["pr"], ["project-000000"])
        self.assertEqual(props["ar"], [])
        self.assertEqual(props["agr"], [])
        self.assertEqual(props["st"], 1)

    def test_edit_notes_replaces_task6_notes_payload(self) -> None:
        state = {
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Task", "ss": 0, "st": 0, "ix": 1},
            }
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target=None,
            notes="Updated notes",
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertIn("nt", props)
        self.assertEqual(props["nt"]["_t"], "tx")
        self.assertEqual(props["nt"]["v"], "Updated notes")

    def test_edit_notes_empty_writes_empty_text_payload(self) -> None:
        state = {
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Task", "ss": 0, "st": 0, "ix": 1},
            }
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target=None,
            notes="",
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["nt"], {"_t": "tx", "t": 1, "ch": 0, "v": ""})

    def test_edit_move_area_from_inbox_sets_anytime(self) -> None:
        state = {
            "area-00000000": {
                "e": "Area3",
                "p": {"tt": "Area", "ix": 10},
            },
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Task", "ss": 0, "st": 0, "ix": 1},
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target="area-000000",
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["ar"], ["area-00000000"])
        self.assertEqual(props["pr"], [])
        self.assertEqual(props["agr"], [])
        self.assertEqual(props["st"], 1)

    def test_edit_move_clear_removes_container_fields(self) -> None:
        state = {
            "project-000000": {
                "e": "Task6",
                "p": {"tt": "Project", "tp": 1, "ss": 0, "st": 1, "ix": 10},
            },
            "heading-000000": {
                "e": "Task6",
                "p": {
                    "tt": "Heading",
                    "tp": 2,
                    "ss": 0,
                    "st": 1,
                    "pr": ["project-000000"],
                    "ix": 11,
                },
            },
            "task-edit-0000": {
                "e": "Task6",
                "p": {
                    "tt": "Task",
                    "ss": 0,
                    "st": 1,
                    "pr": ["project-000000"],
                    "agr": ["heading-000000"],
                    "ix": 12,
                },
            },
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target="clear",
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["pr"], [])
        self.assertEqual(props["ar"], [])
        self.assertEqual(props["agr"], [])

    def test_edit_move_clear_from_inbox_sets_anytime(self) -> None:
        state = {
            "task-edit-0000": {
                "e": "Task6",
                "p": {"tt": "Task", "ss": 0, "st": 0, "ix": 1},
            }
        }
        store = ThingsStore(state)
        client = _FakeClient()
        args = argparse.Namespace(
            task_id="task-edit",
            title=None,
            move_target="clear",
            notes=None,
        )

        out = io.StringIO()
        err = io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            cli.cmd_edit(store, args, cast(Any, client))

        self.assertEqual(err.getvalue(), "")
        self.assertEqual(len(client.calls), 1)
        _task_uuid, props, _entity = client.calls[0]
        self.assertEqual(props["pr"], [])
        self.assertEqual(props["ar"], [])
        self.assertEqual(props["agr"], [])
        self.assertEqual(props["st"], 1)


if __name__ == "__main__":
    unittest.main()
