from tests.helpers import get_fixture, run_cli
from tests.mutating_fixtures import store, tag
from tests.mutating_http_helpers import (
    assert_commit_payloads,
    assert_no_commits,
    p,
    run_cli_mutating_http,
)

NOW = 1_700_000_222.0
TAG_UUID = "AAAAAAAAAAAAAAAAAAAparent"
CHILD_UUID = "BBBBBBBBBBBBBBBBBBBchild1"


def _tag_create(
    uuid: str,
    title: str,
    *,
    ix: int,
    shortcut: str | None = None,
    parent: str | None = None,
) -> dict:
    props = {"tt": title, "ix": ix}
    if shortcut is not None:
        props["sh"] = shortcut
    if parent is not None:
        props["pn"] = [parent]
    return {uuid: {"t": 0, "e": "Tag4", "p": props}}


def test_tags_empty(store_from_journal) -> None:
    assert run_cli("tags", store_from_journal([])) == get_fixture("tags_empty")


def test_tags_basic_list(store_from_journal) -> None:
    journal = [
        _tag_create("GKYVAxEFFoZX9qRavLpSxC", "Home", ix=10),
        _tag_create("5QpptG3mkc9Euc372cZH2X", "Work", ix=20),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_basic_list"
    )


def test_tags_renders_shortcuts(store_from_journal) -> None:
    journal = [
        _tag_create("HJXTqkytEmD1tFNQboJbaK", "Focus", ix=10, shortcut="f"),
        _tag_create("Ai9KrPNZbVwf5VFKDMNLc7", "Home", ix=20),
        _tag_create("DkUdPWL22mk5bkFr5Y7q6t", "Errands", ix=30, shortcut="e"),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_shortcut_rendering"
    )


def test_tags_subtags_under_single_parent(store_from_journal) -> None:
    PARENT = "AAAAAAAAAAAAAAAAAAAparent"
    journal = [
        _tag_create(PARENT, "Work", ix=10),
        _tag_create("BBBBBBBBBBBBBBBBBBBchild1", "Meetings", ix=20, parent=PARENT),
        _tag_create("CCCCCCCCCCCCCCCCCCCchild2", "Projects", ix=30, parent=PARENT),
        _tag_create("DDDDDDDDDDDDDDDDDDDtoplvl2", "Personal", ix=40),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_subtags_single_parent"
    )


def test_tags_subtags_orphan_falls_back_to_top_level(store_from_journal) -> None:
    PARENT = "AAAAAAAAAAAAAAAAAAAparent"
    journal = [
        _tag_create(PARENT, "Places", ix=10),
        _tag_create("BBBBBBBBBBBBBBBBBBBchild1", "Cafe", ix=20, parent=PARENT),
        _tag_create("CCCCCCCCCCCCCCCCCCCchild2", "Restaurant", ix=30, parent=PARENT),
        _tag_create("DDDDDDDDDDDDDDDDDDDchild3", "Bar", ix=40, parent=PARENT),
        _tag_create(
            "EEEEEEEEEEEEEEEEEEEorphan",
            "Orphan",
            ix=50,
            parent="ZZZZZZZZZZZZZZZZZZZmissing",
        ),
        _tag_create("FFFFFFFFFFFFFFFFFFFtoplvl2", "Errands", ix=60),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_subtags_orphan_fallback"
    )


def test_tags_subtags_two_levels_deep(store_from_journal) -> None:
    PLACES = "AAAAAAAAAAAAAAAAAAA1"
    RESTAURANTS = "BBBBBBBBBBBBBBBBBBB2"
    journal = [
        _tag_create(PLACES, "Places", ix=10),
        _tag_create(RESTAURANTS, "Restaurants", ix=20, parent=PLACES),
        _tag_create("CCCCCCCCCCCCCCCCCCC3", "Fine Dining", ix=30, parent=RESTAURANTS),
        _tag_create("DDDDDDDDDDDDDDDDDDD4", "Casual", ix=40, parent=RESTAURANTS),
        _tag_create("EEEEEEEEEEEEEEEEEEE5", "Cafes", ix=50, parent=PLACES),
        _tag_create("FFFFFFFFFFFFFFFFFFF6", "Errands", ix=60),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_subtags_two_levels"
    )


def test_tags_filters_blank_and_whitespace_titles(store_from_journal) -> None:
    journal = [
        _tag_create("5DpSbPqqW43rGrmHeTtpYC", "", ix=5),
        _tag_create("V6ovbTrWN2p5yCNo3GaNPS", "   ", ix=10),
        _tag_create("SodAejXUasPJGBoLKdJ7hy", "Errands", ix=15),
        _tag_create("Ka2MbPmKkLgmR3v3jE6LU9", "\t", ix=20),
        _tag_create("QfcNkgb1LwJqmUyXQhcwGN", "Personal", ix=25),
    ]

    assert run_cli("tags", store_from_journal(journal)) == get_fixture(
        "tags_filter_blank_titles"
    )


NEW_UUID = "CCCCCCCCCCCCCCCCCCCnewuuid"


def test_tags_new_payload() -> None:
    result = run_cli_mutating_http(
        'tags new "Focus"',
        store(),
        extra_patches=[
            p("things_cloud.cli.cmd_tags.random_task_id", return_value=NEW_UUID),
        ],
    )
    assert_commit_payloads(
        result,
        {
            NEW_UUID: {
                "t": 0,
                "e": "Tag4",
                "p": {"tt": "Focus", "ix": 0, "xx": {"_t": "oo", "sn": {}}},
            }
        },
    )


def test_tags_new_with_parent_payload() -> None:
    result = run_cli_mutating_http(
        f'tags new "Meetings" --parent Work',
        store(tag(TAG_UUID, "Work")),
        extra_patches=[
            p("things_cloud.cli.cmd_tags.random_task_id", return_value=NEW_UUID),
        ],
    )
    assert_commit_payloads(
        result,
        {
            NEW_UUID: {
                "t": 0,
                "e": "Tag4",
                "p": {
                    "tt": "Meetings",
                    "ix": 0,
                    "xx": {"_t": "oo", "sn": {}},
                    "pn": [TAG_UUID],
                },
            }
        },
    )


def test_tags_new_empty_name_is_rejected() -> None:
    result = run_cli_mutating_http('tags new "  "', store())
    assert_no_commits(result)
    assert result.stderr == "Tag name cannot be empty.\n"


def test_tags_new_unknown_parent_is_rejected() -> None:
    result = run_cli_mutating_http('tags new "Meetings" --parent nonexistent', store())
    assert_no_commits(result)
    assert result.stderr == "Tag not found: nonexistent\n"


def test_tags_edit_rename_payload() -> None:
    result = run_cli_mutating_http(
        f'tags edit {TAG_UUID} --name "Work Stuff"',
        store(tag(TAG_UUID, "Work")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TAG_UUID: {"t": 1, "e": "Tag4", "p": {"tt": "Work Stuff", "md": NOW}}},
    )


def test_tags_edit_rename_by_title() -> None:
    result = run_cli_mutating_http(
        'tags edit Work --name "Work Stuff"',
        store(tag(TAG_UUID, "Work")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {TAG_UUID: {"t": 1, "e": "Tag4", "p": {"tt": "Work Stuff", "md": NOW}}},
    )


def test_tags_edit_reparent_payload() -> None:
    result = run_cli_mutating_http(
        f"tags edit {CHILD_UUID} --move {TAG_UUID}",
        store(tag(TAG_UUID, "Work"), tag(CHILD_UUID, "Meetings")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {CHILD_UUID: {"t": 1, "e": "Tag4", "p": {"pn": [TAG_UUID], "md": NOW}}},
    )


def test_tags_edit_reparent_by_title() -> None:
    result = run_cli_mutating_http(
        "tags edit Meetings --move Work",
        store(tag(TAG_UUID, "Work"), tag(CHILD_UUID, "Meetings")),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {CHILD_UUID: {"t": 1, "e": "Tag4", "p": {"pn": [TAG_UUID], "md": NOW}}},
    )


def test_tags_edit_clear_parent_payload() -> None:
    result = run_cli_mutating_http(
        f"tags edit {CHILD_UUID} --move clear",
        store(tag(TAG_UUID, "Work"), tag(CHILD_UUID, "Meetings", pn=[TAG_UUID])),
        extra_patches=[p("things_cloud.client.time.time", return_value=NOW)],
    )
    assert_commit_payloads(
        result,
        {CHILD_UUID: {"t": 1, "e": "Tag4", "p": {"pn": [], "md": NOW}}},
    )


def test_tags_edit_no_changes_is_rejected() -> None:
    result = run_cli_mutating_http(
        f"tags edit {TAG_UUID}",
        store(tag(TAG_UUID, "Work")),
    )
    assert_no_commits(result)
    assert result.stderr == "No edit changes requested.\n"


def test_tags_edit_self_parent_is_rejected() -> None:
    result = run_cli_mutating_http(
        f"tags edit {TAG_UUID} --move {TAG_UUID}",
        store(tag(TAG_UUID, "Work")),
    )
    assert_no_commits(result)
    assert result.stderr == "A tag cannot be its own parent.\n"


def test_tags_delete_by_uuid_payload() -> None:
    result = run_cli_mutating_http(
        f"tags delete {TAG_UUID}",
        store(tag(TAG_UUID, "Work")),
    )
    assert_commit_payloads(
        result,
        {TAG_UUID: {"t": 2, "e": "Tag4", "p": {}}},
    )


def test_tags_delete_by_title() -> None:
    result = run_cli_mutating_http(
        "tags delete Work",
        store(tag(TAG_UUID, "Work")),
    )
    assert_commit_payloads(
        result,
        {TAG_UUID: {"t": 2, "e": "Tag4", "p": {}}},
    )


def test_tags_delete_not_found() -> None:
    result = run_cli_mutating_http(
        "tags delete nonexistent",
        store(tag(TAG_UUID, "Work")),
    )
    assert_no_commits(result)
    assert result.stderr == "Tag not found: nonexistent\n"
