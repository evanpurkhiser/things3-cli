from datetime import datetime, timezone

from tests.helpers import get_fixture, run_cli
from tests.mutating_fixtures import store


def _today_ts() -> int:
    return int(
        datetime.now(tz=timezone.utc)
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )


def _task_create(
    uuid: str,
    title: str,
    *,
    ix: int,
    st: int,
    sr: int | None = None,
    tir: int | None = None,
    ti: int = 0,
    sb: int = 0,
    nt: str | dict | None = None,
) -> dict:
    props = {
        "tt": title,
        "tp": 0,
        "ss": 0,
        "st": st,
        "ix": ix,
        "cd": 1,
        "md": 1,
    }
    if sr is not None:
        props["sr"] = sr
    if tir is not None:
        props["tir"] = tir
    if ti:
        props["ti"] = ti
    if sb:
        props["sb"] = sb
    if nt is not None:
        props["nt"] = nt
    return {uuid: {"t": 0, "e": "Task6", "p": props}}


def _checklist_create(
    uuid: str,
    task_uuid: str,
    title: str,
    *,
    ix: int,
    ss: int = 0,
) -> dict:
    return {
        uuid: {
            "t": 0,
            "e": "ChecklistItem3",
            "p": {"tt": title, "ts": [task_uuid], "ss": ss, "ix": ix, "cd": 1, "md": 1},
        }
    }


def test_today_empty(store_from_journal) -> None:
    store = store_from_journal([])
    assert run_cli("today", store) == get_fixture("today_empty")


def test_today_basic_list(store_from_journal) -> None:
    day_ts = _today_ts()
    journal = [
        _task_create(
            "A7h5eCi24RvAWKC3Hv3muf",
            "Morning workout",
            ix=10,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=20,
        ),
        _task_create(
            "KGvAPpMrzHAKMdgMiERP1V",
            "Read email",
            ix=20,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=40,
        ),
    ]

    store = store_from_journal(journal)
    assert run_cli("today", store) == get_fixture("today_basic")


def test_today_evening_section_split(store_from_journal) -> None:
    day_ts = _today_ts()
    journal = [
        _task_create(
            "A7h5eCi24RvAWKC3Hv3muf",
            "Plan day",
            ix=10,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=10,
        ),
        _task_create(
            "KGvAPpMrzHAKMdgMiERP1V",
            "Review finances",
            ix=20,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=20,
            sb=1,
        ),
    ]

    store = store_from_journal(journal)
    assert run_cli("today", store) == get_fixture("today_evening")


def test_today_detailed_with_notes_and_checklist(store_from_journal) -> None:
    day_ts = _today_ts()
    journal = [
        _task_create(
            "A7h5eCi24RvAWKC3Hv3muf",
            "Ship release",
            ix=10,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=10,
            nt={"_t": "tx", "t": 1, "v": "Prep checklist\nNotify stakeholders"},
        ),
        _checklist_create(
            "MpkEei6ybkFS2n6SXvwfLf",
            "A7h5eCi24RvAWKC3Hv3muf",
            "Confirm changelog",
            ix=1,
            ss=0,
        ),
        _checklist_create(
            "JFdhhhp37fpryAKu8UXwzK",
            "A7h5eCi24RvAWKC3Hv3muf",
            "Tag release commit",
            ix=2,
            ss=3,
        ),
    ]

    store = store_from_journal(journal)
    assert run_cli("today --detailed", store) == get_fixture("today_detailed")


def test_today_marks_staged_someday_items(store_from_journal) -> None:
    day_ts = _today_ts()
    journal = [
        _task_create(
            "A7h5eCi24RvAWKC3Hv3muf",
            "Call back eye doctor",
            ix=10,
            st=2,
            sr=day_ts,
            tir=day_ts,
            ti=10,
        ),
        _task_create(
            "KGvAPpMrzHAKMdgMiERP1V",
            "Set up git config to use GitHub auth token on server",
            ix=20,
            st=1,
            sr=day_ts,
            tir=day_ts,
            ti=20,
        ),
    ]

    store = store_from_journal(journal)
    assert run_cli("today", store) == get_fixture("today_staged")


def test_no_args_defaults_to_today() -> None:
    """Invoking with no arguments should show the Today view without error."""
    assert "No tasks for today." in run_cli("", store())
