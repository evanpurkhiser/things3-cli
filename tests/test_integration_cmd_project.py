from collections.abc import Callable

from things_cloud.store import ThingsStore

from tests.helpers import get_fixture, run_cli


def _task_create(
    uuid: str,
    title: str,
    *,
    ix: int,
    st: int = 1,
    ss: int = 0,
    tp: int = 0,
    pr: str | None = None,
    agr: str | None = None,
    nt: dict | None = None,
) -> dict:
    props: dict = {
        "tt": title,
        "tp": tp,
        "ss": ss,
        "st": st,
        "ix": ix,
        "cd": 1710000000,
        "md": 1710000000,
    }
    if pr is not None:
        props["pr"] = [pr]
    if agr is not None:
        props["agr"] = [agr]
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
            "p": {
                "tt": title,
                "ts": [task_uuid],
                "ss": ss,
                "ix": ix,
                "cd": 1710000000,
                "md": 1710000000,
            },
        }
    }


def test_project_not_found_has_no_stdout(
    store_from_journal: Callable[[list[dict]], ThingsStore],
) -> None:
    journal = [_task_create("Dxf7yNCKaPoWNBVM7zVi2p", "Kitchen Refresh", ix=10, tp=1)]

    assert run_cli("project zzzzz", store_from_journal(journal)) == ""


def test_project_empty(
    store_from_journal: Callable[[list[dict]], ThingsStore],
) -> None:
    journal = [
        _task_create("Dxf7yNCKaPoWNBVM7zVi2p", "Backyard Renovation", ix=10, tp=1)
    ]

    assert run_cli("project D", store_from_journal(journal)) == get_fixture(
        "project_empty"
    )


def test_project_grouped_with_progress_counts(
    store_from_journal: Callable[[list[dict]], ThingsStore],
) -> None:
    journal = [
        _task_create("Dxf7yNCKaPoWNBVM7zVi2p", "Release Plan", ix=10, tp=1),
        _task_create(
            "Lkf6UBiZ8vUzc8qQBSVgmo",
            "Draft announcement",
            ix=10,
            pr="Dxf7yNCKaPoWNBVM7zVi2p",
        ),
        _task_create(
            "7WqeVvEgnQxNLkTxrVBDCn",
            "Publish release notes",
            ix=20,
            pr="Dxf7yNCKaPoWNBVM7zVi2p",
            ss=3,
        ),
        _task_create(
            "VPBKRqFVE5ovBe8U5gCNfX",
            "Quality Assurance",
            ix=100,
            tp=2,
            pr="Dxf7yNCKaPoWNBVM7zVi2p",
        ),
        _task_create(
            "DTBiViWPQEgV8biabTY3kH",
            "Run regression suite",
            ix=110,
            agr="VPBKRqFVE5ovBe8U5gCNfX",
        ),
        _task_create(
            "XUEGwoo1X9Kz1myotYqHLv",
            "Capture screenshots",
            ix=120,
            agr="VPBKRqFVE5ovBe8U5gCNfX",
        ),
    ]

    assert run_cli("project Dx", store_from_journal(journal)) == get_fixture(
        "project_grouped"
    )


def test_project_detailed_with_notes_and_checklist(
    store_from_journal: Callable[[list[dict]], ThingsStore],
) -> None:
    journal = [
        _task_create("Dxf7yNCKaPoWNBVM7zVi2p", "Conference Trip", ix=10, tp=1),
        _task_create(
            "GXNeg2wqB1B5diQduwUykr",
            "Finalize packing",
            ix=10,
            pr="Dxf7yNCKaPoWNBVM7zVi2p",
            nt={"_t": "tx", "t": 1, "v": "Bring carry-on only\nCharge battery pack"},
        ),
        _checklist_create(
            "9ZZLUGDsHVEgh5LpLPzzvu", "GXNeg2wqB1B5diQduwUykr", "Passport", ix=10
        ),
        _checklist_create(
            "SA4EbQAPS5a8e5s6ZMfFvT",
            "GXNeg2wqB1B5diQduwUykr",
            "Headphones",
            ix=20,
            ss=3,
        ),
    ]

    assert run_cli("project D --detailed", store_from_journal(journal)) == get_fixture(
        "project_detailed"
    )
