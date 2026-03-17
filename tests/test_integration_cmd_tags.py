from tests.helpers import get_fixture, run_cli


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
