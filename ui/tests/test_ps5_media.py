import pytest
from looks import invoke, read
from PySide6.QtCore import Qt

from conftest import record, until

pytestmark = pytest.mark.parametrize("look", ["ps5"], indirect=True)
MINE = ("MediaGalleryPage", "PlayerPage", "NewsPage", "ArticlePage", "Gallery", "JournalCard")


@pytest.fixture(autouse=True)
def quiet(look):
    warnings = record(look.engine.warnings)
    yield
    assert [w.toString() for (ws,) in warnings for w in ws if any(name in w.toString() for name in MINE)] == []


def gallery(api, look, args=None):
    page = look.open("pages/MediaGalleryPage.qml", args)
    until(lambda: not api.screens.media.loading and api.screens.media.rows)
    return page


def test_the_gallery_holds_every_kind_and_each_tab_one(api, look):
    page = gallery(api, look)
    shown = read(page, "shown")
    assert {r["kind"] for r in shown} == {"shot", "recording", "journal"} and len(shown) == len(api.screens.media.rows)
    assert [r["when"] for r in shown] == sorted((r["when"] for r in shown), reverse=True), "newest first"
    for tab, kind in ((1, "shot"), (2, "recording"), (3, "journal")):
        look.press(Qt.Key.Key_E)
        assert page.property("tab") == tab
        assert {r["kind"] for r in read(page, "shown")} == {kind}
    look.press(Qt.Key.Key_Q, 3)
    assert page.property("tab") == 0


def test_a_game_named_filters_the_gallery(api, look):
    page = gallery(api, look, {"gameId": "the-technomancer"})
    shown = read(page, "shown")
    assert shown and all(r["gameId"] == "the-technomancer" for r in shown) and page.property("filterId") == "the-technomancer"
    look.root.pop()
    other = gallery(api, look, {"gameId": "dead-cells"})
    assert read(other, "shown") == [], "a game with no captures shows none"


def test_a_shot_opens_full_screen_and_steps_through_the_shots(api, look):
    page = gallery(api, look)
    look.press(Qt.Key.Key_E)
    look.press(Qt.Key.Key_Return)
    assert page.property("viewing") is True and page.property("shotIndex") == 0
    look.press(Qt.Key.Key_Right)
    assert page.property("shotIndex") == 1 and page.property("index") == 1, "the grid follows the picture"
    look.press(Qt.Key.Key_Escape)
    assert page.property("viewing") is False and look.root.property("depth") == 1, "B closes the picture, not the page"


def test_a_recording_opens_the_player_which_finds_its_row_cold(api, look):
    api.screens.album.load("dead-cells")
    page = gallery(api, look)
    look.press(Qt.Key.Key_E, 2)
    session = read(page, "current")["session"]
    look.press(Qt.Key.Key_Return)
    player = look.page("playerPage")
    assert player.property("strip") is True
    row = until(lambda: read(player, "row"), "the album was asked for the game's rows")
    assert row["session"] == session and api.screens.album.gameId == "the-technomancer"
    until(lambda: player.property("activeFocus"))
    look.press(Qt.Key.Key_Escape)
    until(lambda: look.root.property("depth") == 1)


def test_a_journal_row_opens_its_article(api, look):
    page = gallery(api, look)
    look.press(Qt.Key.Key_E, 3)
    session = read(page, "current")["session"]
    look.press(Qt.Key.Key_Return)
    row = until(lambda: read(look.page("articlePage"), "row"))
    assert row["session"] == session and row["state"] == "written"


def test_a_capture_named_by_the_hub_opens_directly(api, look):
    media = api.universe.media("the-technomancer")
    shot = [r for r in media if r["kind"] == "shot"][1]
    page = gallery(api, look, {"gameId": "the-technomancer", "path": shot["path"], "session": shot["session"]})
    until(lambda: page.property("viewing") is True and read(page, "current")["path"] == shot["path"], "a shot opens full screen")
    look.root.pop()
    recording = next(r for r in media if r["kind"] == "recording")
    invoke(look.root, "push", "pages/MediaGalleryPage.qml", {"gameId": "the-technomancer", "path": recording["path"], "session": recording["session"]})
    row = until(lambda: read(look.page("playerPage"), "row"), "a recording opens in the player")
    assert row["session"] == recording["session"]


def test_an_article_pushed_cold_loads_its_game(api, look):
    api.screens.news.load("dead-cells")
    article = look.open("pages/ArticlePage.qml", {"gameId": "the-technomancer", "session": "20260907-224100"})
    row = until(lambda: read(article, "row"))
    assert row["title"] == "Ophir, first night" and row["next_up"] != ""
    assert api.screens.news.gameId == "the-technomancer"


def test_the_journal_lists_every_session_and_its_state(look):
    page = look.open("pages/NewsPage.qml")
    entries = until(lambda: read(page, "entries"))
    assert [e["state"] for e in entries] == ["written", "written", "none", "deferred"]
    look.press(Qt.Key.Key_F1)
    assert look.root.property("modal") is True, "Start opens the entry's menu"
    look.press(Qt.Key.Key_Escape)
    assert look.root.property("modal") is False
    look.press(Qt.Key.Key_Return)
    row = until(lambda: read(look.page("articlePage"), "row"))
    assert row["session"] == entries[0]["session"]
