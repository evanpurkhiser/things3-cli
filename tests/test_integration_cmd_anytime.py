from datetime import datetime, timedelta, timezone

from tests.helpers import get_fixture, run_cli


def _day_ts(offset_days: int = 0) -> int:
    return int(
        (datetime.now(tz=timezone.utc) + timedelta(days=offset_days))
        .replace(hour=0, minute=0, second=0, microsecond=0)
        .timestamp()
    )


def _task_create(
    uuid: str,
    title: str,
    *,
    ix: int,
    tp: int = 0,
    st: int = 1,
    ss: int = 0,
    sr: int | None = None,
    tr: bool = False,
    nt: str | dict | None = None,
    pr: list[str] | None = None,
    ar: list[str] | None = None,
) -> dict:
    props = {
        "tt": title,
        "tp": tp,
        "ss": ss,
        "st": st,
        "ix": ix,
        "cd": 1,
        "md": 1,
    }
    if sr is not None:
        props["sr"] = sr
    if tr:
        props["tr"] = True
    if nt is not None:
        props["nt"] = nt
    if pr is not None:
        props["pr"] = pr
    if ar is not None:
        props["ar"] = ar
    return {uuid: {"t": 0, "e": "Task6", "p": props}}


def _area_create(uuid: str, title: str, *, ix: int) -> dict:
    return {
        uuid: {
            "t": 0,
            "e": "Area3",
            "p": {"tt": title, "ix": ix},
        }
    }


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


def test_anytime_empty(store_from_journal) -> None:
    assert run_cli("anytime", store_from_journal([])) == get_fixture("anytime_empty")


def test_anytime_basic_list(store_from_journal) -> None:
    day_ts = _day_ts()
    journal = [
        _task_create("6aXZoaKdhWbtkVDjkjSh6t", "Draft roadmap", ix=10),
        _task_create("3eyRB1WYUNtkYfE8B3MGPn", "Pay rent", ix=20, sr=day_ts),
    ]

    assert run_cli("anytime", store_from_journal(journal)) == get_fixture(
        "anytime_basic"
    )


def test_anytime_filters_someday_future_trashed_and_completed(
    store_from_journal,
) -> None:
    journal = [
        _task_create("6aXZoaKdhWbtkVDjkjSh6t", "Visible anytime task", ix=10),
        _task_create("JGHbpq9qT112kF3pMfHYVN", "Someday backlog", ix=20, st=2),
        _task_create(
            "EVm4iCcMXiBp4eWKojk2zp", "Future scheduled", ix=30, sr=_day_ts(1)
        ),
        _task_create("4LHrEe3jyYApPfnNPMPpxn", "Trashed task", ix=40, tr=True),
        _task_create("QSHXpCLatmt3h9DskZ1RMF", "Completed task", ix=50, ss=3),
    ]

    assert run_cli("anytime", store_from_journal(journal)) == get_fixture(
        "anytime_filtered"
    )


def test_anytime_detailed_with_notes_and_checklist(store_from_journal) -> None:
    journal = [
        _task_create(
            "6aXZoaKdhWbtkVDjkjSh6t",
            "Prepare trip plan",
            ix=10,
            nt={"_t": "tx", "t": 1, "v": "Book train tickets\nPack carry-on only"},
        ),
        _checklist_create(
            "LK55LNQ2Th3Tdx2qi161pM",
            "6aXZoaKdhWbtkVDjkjSh6t",
            "Confirm passport expiry",
            ix=1,
            ss=0,
        ),
        _checklist_create(
            "CwqFCJUboRLmL8E2D7J24f",
            "6aXZoaKdhWbtkVDjkjSh6t",
            "Download offline maps",
            ix=2,
            ss=3,
        ),
    ]

    assert run_cli("anytime --detailed", store_from_journal(journal)) == get_fixture(
        "anytime_detailed"
    )


def test_anytime_groups_by_area_and_project_with_hiding(store_from_journal) -> None:
    area_uuid = "B1111111111111111111111"
    project_uuid = "C1111111111111111111111"
    project_no_area_uuid = "P1111111111111111111111"
    journal = [
        _task_create("A1111111111111111111111", "Loose task", ix=10),
        _area_create(area_uuid, "Home", ix=1),
        _task_create(
            project_uuid,
            "Renovation",
            ix=15,
            tp=1,
            ar=[area_uuid],
        ),
        _task_create(project_no_area_uuid, "Errands", ix=17, tp=1),
        _task_create("D1111111111111111111111", "Area task", ix=20, ar=[area_uuid]),
        _task_create(
            "I1111111111111111111111",
            "Project-only task 1",
            ix=25,
            pr=[project_no_area_uuid],
        ),
        _task_create(
            "J1111111111111111111111",
            "Project-only task 2",
            ix=26,
            pr=[project_no_area_uuid],
        ),
        _task_create(
            "E1111111111111111111111", "Project task 1", ix=30, pr=[project_uuid]
        ),
        _task_create(
            "F1111111111111111111111", "Project task 2", ix=40, pr=[project_uuid]
        ),
        _task_create(
            "G1111111111111111111111", "Project task 3", ix=50, pr=[project_uuid]
        ),
        _task_create(
            "H1111111111111111111111",
            "Project task 4 (hidden)",
            ix=60,
            pr=[project_uuid],
        ),
    ]

    assert run_cli("anytime", store_from_journal(journal)) == get_fixture(
        "anytime_grouped_hiding"
    )
