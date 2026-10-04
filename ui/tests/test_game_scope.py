import os

import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_render import render, settle

from conftest import pump, record, until
from universe_ui import host

LOOKS = ["reprise", "ps5", "switch2"]
# The looks with quick settings over the game: Reprise's dock, the PS5 look's Control Center.
OVERLAYS = {"reprise": "dockScope", "ps5": "ccScope"}


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


@pytest.fixture
def game_page(api):
    """Opens a game's settings page in a look, the cursor landed on `key`: (window, page, hints of what is on top)."""
    opened = []

    def open_page(look, game_id, key):
        api.theme.set(look)
        api.theme.takeLanding()
        engine, window = render(api, activate=True)
        opened.append((engine, window))
        root = window.property("contentItem").childItems()[0].property("item")
        if look == "reprise":
            root.openSub("pages/GameSettingsPage.qml", {"game": api.allGames.byId(game_id), "key": key})
            page = until(lambda: window.findChild(QObject, "gameSettingsPage"))
            until(lambda: (value(page, "row") or {}).get("key") == key, f"landed on {key}")
            return window, page, lambda: value(page, "hints")
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/GameSettingsPage.qml"), Q_ARG("QVariant", {"gameId": game_id, "key": key}))
        page = until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)
        until(lambda: (value(page, "currentRow") or {}).get("key") == key, f"landed on {key}")
        return window, page, lambda: value(root, "hints")

    yield open_page
    for _engine, window in opened:
        window.close()


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)


def glyphs(hints):
    return [h["glyph"] for h in hints()]


@pytest.mark.parametrize("look", LOOKS)
def test_y_in_a_games_picker_gives_the_value_to_every_game_it_reaches(game_page, api, fake, look):
    window, _page, hints = game_page(look, "the-technomancer", "launch.gamescope_scaler")
    told = record(api.screens.gameSettings.message)
    click(window, Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(hints), "the picker offers all games on Y")
    click(window, Qt.Key.Key_Down, 2)
    click(window, Qt.Key.Key_F)
    until(lambda: (fake.config()["set"].get("launch") or {}).get("gamescope_scaler") == "integer", "Y wrote the global value")
    assert "gamescope_scaler" not in fake.game("the-technomancer")["launch"], "the game follows it"
    until(lambda: len(told) == 1)
    click(window, Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(hints))
    click(window, Qt.Key.Key_Down)
    click(window, Qt.Key.Key_Return)
    until(lambda: fake.game("the-technomancer")["launch"].get("gamescope_scaler") == "auto", "A keeps the pick to the game")
    assert fake.config()["set"]["launch"]["gamescope_scaler"] == "integer"


@pytest.mark.parametrize("look", LOOKS)
def test_a_typed_value_then_asks_which_games_it_is_for(game_page, fake, look):
    fake.core.set("the-technomancer", "launch.gamescope_args", "--expose-wayland")
    window, _page, hints = game_page(look, "the-technomancer", "launch.gamescope_args")
    before = glyphs(hints)
    click(window, Qt.Key.Key_Return)
    until(lambda: glyphs(hints) != before, "the keyboard is up")
    click(window, Qt.Key.Key_Return if look == "reprise" else Qt.Key.Key_F1)
    until(lambda: "Y" in glyphs(hints), "done typing: this game on A, all games on Y")
    click(window, Qt.Key.Key_F)
    until(lambda: (fake.config()["set"].get("launch") or {}).get("gamescope_args") == "--expose-wayland")
    assert "gamescope_args" not in fake.game("the-technomancer")["launch"]


@pytest.mark.parametrize("look", LOOKS)
def test_a_typed_value_for_all_games_given_up_leaves_the_next_one_asking(game_page, fake, look):
    fake.core.set("the-technomancer", "launch.gamescope_args", "--expose-wayland")
    window, page, hints = game_page(look, "the-technomancer", "launch.gamescope_resolution")
    landed = "row" if look == "reprise" else "currentRow"
    before = glyphs(hints)
    click(window, Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(hints), "the picker offers all games on Y")
    picking = glyphs(hints)
    click(window, Qt.Key.Key_Down, len(value(page, landed)["choices"]) + 1)
    click(window, Qt.Key.Key_F)
    until(lambda: glyphs(hints) not in (before, picking), "Y on Type a value…: the keyboard is up")
    click(window, Qt.Key.Key_Escape if look == "reprise" else Qt.Key.Key_I)
    until(lambda: glyphs(hints) == before, "given up")
    page.setProperty("landKey", "launch.gamescope_args")
    QMetaObject.invokeMethod(page, "landNow")
    until(lambda: (value(page, landed) or {}).get("key") == "launch.gamescope_args")
    click(window, Qt.Key.Key_Return)
    until(lambda: glyphs(hints) != before, "the keyboard is up")
    click(window, Qt.Key.Key_Return if look == "reprise" else Qt.Key.Key_F1)
    until(lambda: "Y" in glyphs(hints), "this game on A, all games on Y: nothing went to every game unasked")
    assert "gamescope_args" not in (fake.config()["set"].get("launch") or {})
    click(window, Qt.Key.Key_Escape)


@pytest.mark.parametrize("look", LOOKS)
def test_a_switch_flips_here_on_a_and_for_every_game_from_the_options(game_page, api, fake, look):
    window, _page, hints = game_page(look, "the-technomancer", "launch.mangohud")
    told = record(api.screens.gameSettings.message)
    before = glyphs(hints)
    click(window, Qt.Key.Key_F1)
    until(lambda: glyphs(hints) != before, "the options are up")
    click(window, Qt.Key.Key_Return)
    until(lambda: (fake.config()["set"].get("launch") or {}).get("mangohud") is True, "the first option flips it for every game")
    assert "mangohud" not in fake.game("the-technomancer")["launch"] and len(told) == 1


@pytest.mark.parametrize("look", LOOKS)
def test_a_value_only_the_game_holds_offers_no_other_games(game_page, fake, look):
    window, _page, hints = game_page(look, "the-technomancer", "launch.runner")
    before = glyphs(hints)
    click(window, Qt.Key.Key_Return)
    until(lambda: glyphs(hints) != before, "the picker is up")
    assert "Y" not in glyphs(hints), "the runner is the game's alone"
    click(window, Qt.Key.Key_Escape)


def land(api, look, overlay, group, row):
    """Opens the quick settings over the running game on one row of a group: (panel, the scope line)."""
    api.home.openDock()
    overlay.requestActivate()
    until(overlay.isActive)
    panel = overlay.property("contentItem").childItems()[0].property("item")
    until(lambda: panel.property("open") is True)
    if look == "reprise":
        panel.setProperty("index", [b["id"] for b in value(panel, "buttons")].index(group))
        click(overlay, Qt.Key.Key_Return)
        until(lambda: panel.property("opened") is True)
        panel.setProperty("sub", [c["id"] for c in value(panel, "current")["children"]].index(row))
    else:
        panel.setProperty("icon", [i["id"] for i in value(panel, "icons")].index(group))
        panel.setProperty("zone", "panel")
        panel.setProperty("row", [r["id"] for r in value(panel, "panelRows")].index(row))
    scope = overlay.findChild(QObject, OVERLAYS[look])
    until(lambda: scope.property("visible") is True, "the panel says the setting is this game's and Y gives it to all")
    return panel, scope


@pytest.fixture
def overlay(api):
    """A look's window and the overlay its quick settings draw on, once a game runs."""
    opened = []

    def open_overlay(look):
        api.theme.set(look)
        api.theme.takeLanding()
        engine, window = render(api)
        layer = host.create_overlay(engine, window.size())
        api.home.attachOverlay(layer)
        layer.show()
        opened.append((engine, window, layer))
        settle(layer)
        until(lambda: api.home.shown == "game")
        return layer

    yield open_overlay
    if api.universe.currentSession is not None:
        api.home.stop()
        until(lambda: api.universe.currentSession is None)
    for _engine, window, layer in opened:
        window.close()
        layer.close()


@pytest.mark.parametrize("look", list(OVERLAYS))
def test_a_quick_setting_is_the_games_and_y_gives_it_to_every_game(overlay, api, fake, look):
    fake.launch("mirrors-edge", "")
    overlay = overlay(look)
    panel, scope = land(api, look, overlay, "perf", "fps")
    if where := os.environ.get("UNIVERSE_TEST_SHOTS"):
        until(lambda: panel.property("shown") is True and scope.property("opacity") == 1)
        pump(800)
        overlay.grabWindow().save(os.path.join(where, f"quick-settings-{look}.png"))
    click(overlay, Qt.Key.Key_Right)
    picked = until(lambda: (fake.game("mirrors-edge").get("launch") or {}).get("fps_limit"), "a step is this game's")
    click(overlay, Qt.Key.Key_F)
    until(lambda: (fake.config()["set"].get("launch") or {}).get("fps_limit") == picked, "Y: every game's")
    assert "fps_limit" not in fake.game("mirrors-edge")["launch"], "the game follows it"


@pytest.mark.parametrize("look", list(OVERLAYS))
def test_a_decks_control_set_in_a_game_is_its_own_until_y_makes_it_the_machines(monkeypatch, overlay, api, fake, look):
    monkeypatch.setenv("UNIVERSE_DECK", "oled")
    api.system.reload()
    until(lambda: (api.system.control("tdp") or {}).get("value") == "15")

    def own():
        return (fake.game("mirrors-edge").get("system") or {}).get("tdp")

    def watts():
        return api.system.control("tdp")["value"]

    fake.launch("mirrors-edge", "")
    overlay = overlay(look)
    land(api, look, overlay, "system", "sys_tdp")
    click(overlay, Qt.Key.Key_Left)
    until(lambda: own() == "14", "a step is this game's")
    api.home.stop()
    until(lambda: api.universe.currentSession is None and watts() == "15", "its end puts the machine's own back")
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game" and watts() == "14", "its next launch brings the game's back")
    land(api, look, overlay, "system", "sys_tdp")
    click(overlay, Qt.Key.Key_F)
    until(lambda: own() is None, "Y: the game lets its own go")
    api.home.stop()
    until(lambda: api.universe.currentSession is None)
    assert watts() == "14", "the machine's own now, kept after the game"
