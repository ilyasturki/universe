import pytest
from looks import invoke, read
from PySide6.QtCore import Qt
from uitest import record, until

pytestmark = pytest.mark.parametrize("look", ["ps5"], indirect=True)


def open_store(look, args=None):
    store = look.open("pages/InstallPage.qml", args)
    sources = store.property("sources")
    until(lambda: not sources.property("busy") and sources.property("rows"))
    return store


def titles(store, collection):
    coll = next(c for c in read(store, "collections") if c["id"] == collection)
    listed = store.property("sources").property("rows")
    return [listed[int(i)]["title"] for i in coll["items"]]


def open_update_card(look, store):
    look.press(Qt.Key.Key_Right)
    until(lambda: read(store, "current")["title"] == "Stardew Valley")
    look.press(Qt.Key.Key_Down)
    until(lambda: store.property("row") == 1 and read(store, "current")["title"] == "Mini Metro")
    look.press(Qt.Key.Key_Return)
    until(lambda: store.property("zone") == "card")


def opened_menu(look):
    popup = look.menu()
    until(lambda: popup.property("open") is True)
    return [i["act"] for i in read(popup, "items")]


def test_the_store_lays_the_catalogue_out_in_collections(look):
    store = open_store(look)
    assert store.property("strip") is False, "the console has a store: no hint strip"
    assert {c["id"] for c in read(store, "collections")} == {"ready", "updates", "installed"}
    assert titles(store, "ready") == ["Disco Elysium", "Stardew Valley", "The Witcher 3: Wild Hunt"]
    assert titles(store, "updates") == ["Mini Metro"]
    assert "The Technomancer" in titles(store, "installed")
    assert read(store, "current")["title"] == "Disco Elysium"


def test_the_cursor_walks_the_collections_to_a_game_card_whose_more_is_the_game_s_shared_menu(look):
    store = open_store(look)
    open_update_card(look, store)
    assert read(store, "cardEntry")["title"] == "Mini Metro"
    assert [a["act"] for a in read(store, "cardActions")] == ["update", "more"], "an update waits: Update, the rest under …"
    look.press(Qt.Key.Key_Right)
    look.press(Qt.Key.Key_Return)
    acts = opened_menu(look)
    assert {"play", "info", "settings", "data"} <= set(acts) and acts[-2:] == ["uninstall", "remove"], "the hero's and the Library's menu"
    look.press(Qt.Key.Key_Escape)
    until(lambda: look.menu().property("open") is False)
    assert store.property("zone") == "card"
    look.press(Qt.Key.Key_Escape)
    until(lambda: store.property("zone") == "main", "B closes the card, not the page")
    assert look.root.property("depth") == 1, "B closes the card, not the page"
    look.press(Qt.Key.Key_Up, 2)
    until(lambda: store.property("zone") == "top")


def test_start_opens_every_action_the_focused_game_s_first_and_play_only_once(look):
    store = open_store(look)
    look.press(Qt.Key.Key_F1)
    acts = opened_menu(look)
    assert look.root.property("modal") is True
    assert acts[0] == "resume-download", "the focused game's action first: Disco Elysium is paused"
    assert {"search", "update-all", "tab", "refresh", "sources"} <= set(acts)
    look.press(Qt.Key.Key_Escape)
    until(lambda: look.menu().property("open") is False)
    assert store.property("zone") == "main"
    look.press(Qt.Key.Key_Down, 2)
    until(lambda: store.property("row") == 2 and read(store, "current")["installed"] and not read(store, "current")["pending"])
    look.press(Qt.Key.Key_F1)
    acts = opened_menu(look)
    assert acts[0] == "play" and acts.count("play") == 1


def test_the_card_menu_s_resume_goes_back_to_the_running_game(api, look):
    invoke(look.root, "launch", api.allGames.byId("mini-metro"))
    until(lambda: api.home.shown == "game")
    api.home.toLauncher()
    until(lambda: api.home.shown == "launcher")
    store = open_store(look)
    open_update_card(look, store)
    look.press(Qt.Key.Key_Right)
    look.press(Qt.Key.Key_Return)
    assert opened_menu(look).index("resume") == 0
    look.press(Qt.Key.Key_Return)
    until(lambda: api.home.shown == "game", "the game comes back, no download starts")
    assert not api.screens.sources.job


def test_a_search_narrows_the_store_to_its_results_and_b_clears_it(api, look):
    sources = api.screens.sources
    store = open_store(look)
    sources.search("witcher")
    until(lambda: not sources.busy and [c["id"] for c in read(store, "collections")] == ["results"])
    found = titles(store, "results")
    assert len(found) == len(sources.rows) and all("Witcher" in t for t in found)
    assert "The Witcher: Enhanced Edition" in found and read(store, "current")["title"] == found[0]
    look.press(Qt.Key.Key_Escape)
    until(lambda: sources.query == "", "B drops the search first")
    assert look.root.property("depth") == 1, "B drops the search first"
    until(lambda: read(store, "collections")[0]["id"] == "ready")


def test_an_install_asks_first_and_its_download_heads_the_home_row(api, look, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.15)
    sources = api.screens.sources
    store = open_store(look)
    look.press(Qt.Key.Key_Right)
    look.press(Qt.Key.Key_Return)
    until(lambda: store.property("zone") == "card")
    assert read(store, "cardActions")[0]["act"] == "install"
    look.press(Qt.Key.Key_Return)
    until(lambda: look.root.property("modal") is True, "an install asks first, with the sizes")
    look.press(Qt.Key.Key_Return)
    job = until(lambda: sources.job)
    assert job["ok"] is None and job["title"] == "Stardew Valley"
    assert sources.arriving is not None and sources.arriving.property("installing")
    entries = read(look.find("homePage"), "gameEntries")
    assert entries[1]["game"].property("id") == sources.arriving.property("id"), "the download heads the home row"
    finished = record(fake.jobFinished)
    assert sources.cancel()
    until(lambda: finished)


def test_downloads_opens_with_tab_one_and_the_bumpers_switch_tabs(look):
    store = open_store(look, {"tab": 1})
    assert store.property("tab") == 1
    assert read(store, "line")["game"]["title"] == "Disco Elysium", "the cursor on the first line under its heading"
    look.press(Qt.Key.Key_Down)
    until(lambda: read(store, "line").get("all") is True, "Update everything under the Updates heading")
    look.press(Qt.Key.Key_Q)
    until(lambda: store.property("tab") == 0)
    look.press(Qt.Key.Key_E)
    until(lambda: store.property("tab") == 1)


def test_a_second_store_puts_a_switch_on_top_whose_menu_lists_the_stores(api, look, fake):
    fake.core._source("epic").update(enabled=True, logged_in=True)
    fake.core._data["source_library"]["epic"] = [{"id": "Min", "title": "Hades", "owned": True, "installed": False}]
    store = open_store(look)
    until(lambda: "store" in [i["id"] for i in read(store, "icons")])
    invoke(store, "openStores")
    popup = look.menu()
    until(lambda: popup.property("open") is True)
    assert [i["glyph"] for i in read(popup, "items")] == ["check", "store"], "a row per store, the one shown checked"
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    until(lambda: api.screens.sources.source == "epic")
