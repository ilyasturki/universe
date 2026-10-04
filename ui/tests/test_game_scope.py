import os

import pytest
from looks import current_row, invoke, read, settle
from PySide6.QtCore import QObject, Qt
from PySide6.QtTest import QTest
from uitest import own, record, until

from universe_ui import host

# What closes each look's keyboard on the value typed, and what gives it up.
DONE = {"reprise": Qt.Key.Key_Return, "switch2": Qt.Key.Key_F1, "ps5": Qt.Key.Key_F1}
GIVE_UP = {"reprise": Qt.Key.Key_Escape, "switch2": Qt.Key.Key_I, "ps5": Qt.Key.Key_I}
# The looks with quick settings over the game: Reprise's dock, the PS5 look's Control Center.
PANEL = {"reprise": "dock", "ps5": "controlCenter"}
SCOPE = {"reprise": "dockScope", "ps5": "ccScope"}


def everyones(fake, key):
    return (fake.config()["set"].get("launch") or {}).get(key)


def game_settings(look, key):
    """The Technomancer's settings page, the cursor landed on `key`."""
    page = look.open("pages/GameSettingsPage.qml", {**look.game("the-technomancer"), "key": key})
    until(lambda: page.property("activeFocus") and current_row(page).get("key") == key, f"landed on {key}")
    return page


def glyphs(look, page):
    """The buttons on offer: the page's own in Reprise, the shell's (its dialogs and pickers too) on the stack looks."""
    return [h["glyph"] for h in read(look.root if look.stacked else page, "hints")]


def test_y_in_a_games_picker_gives_the_value_to_every_game_it_reaches(look, api, fake):
    page = game_settings(look, "launch.gamescope_scaler")
    told = record(api.screens.gameSettings.message)
    look.press(Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(look, page), "the picker offers all games on Y")
    look.press(Qt.Key.Key_Down, 2)
    look.press(Qt.Key.Key_F)
    until(lambda: everyones(fake, "gamescope_scaler") == "integer", "Y wrote the global value")
    assert own(fake, "gamescope_scaler") is None, "the game follows it"
    until(lambda: len(told) == 1)
    look.press(Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(look, page))
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    until(lambda: own(fake, "gamescope_scaler") == "auto", "A keeps the pick to the game")
    assert everyones(fake, "gamescope_scaler") == "integer"


def test_a_typed_value_then_asks_which_games_it_is_for(look, fake):
    fake.core.set("the-technomancer", "launch.gamescope_args", "--expose-wayland")
    page = game_settings(look, "launch.gamescope_args")
    before = glyphs(look, page)
    look.press(Qt.Key.Key_Return)
    until(lambda: glyphs(look, page) != before, "the keyboard is up")
    look.press(DONE[look.name])
    until(lambda: "Y" in glyphs(look, page), "done typing: this game on A, all games on Y")
    look.press(Qt.Key.Key_F)
    until(lambda: everyones(fake, "gamescope_args") == "--expose-wayland")
    assert own(fake, "gamescope_args") is None


def test_a_typed_value_for_all_games_given_up_leaves_the_next_one_asking(look, fake):
    fake.core.set("the-technomancer", "launch.gamescope_args", "--expose-wayland")
    page = game_settings(look, "launch.gamescope_resolution")
    before = glyphs(look, page)
    look.press(Qt.Key.Key_Return)
    until(lambda: "Y" in glyphs(look, page), "the picker offers all games on Y")
    picking = glyphs(look, page)
    look.press(Qt.Key.Key_Down, len(current_row(page)["choices"]) + 1)
    look.press(Qt.Key.Key_F)
    until(lambda: glyphs(look, page) not in (before, picking), "Y on Type a value…: the keyboard is up")
    look.press(GIVE_UP[look.name])
    until(lambda: glyphs(look, page) == before, "given up")
    page.setProperty("landKey", "launch.gamescope_args")
    invoke(page, "landNow")
    until(lambda: current_row(page).get("key") == "launch.gamescope_args")
    look.press(Qt.Key.Key_Return)
    until(lambda: glyphs(look, page) != before, "the keyboard is up")
    look.press(DONE[look.name])
    until(lambda: "Y" in glyphs(look, page), "this game on A, all games on Y: nothing went to every game unasked")
    assert everyones(fake, "gamescope_args") is None
    look.press(Qt.Key.Key_Escape)


def test_a_switch_flips_here_on_a_and_for_every_game_from_the_options(look, api, fake):
    page = game_settings(look, "launch.mangohud")
    told = record(api.screens.gameSettings.message)
    before = glyphs(look, page)
    look.press(Qt.Key.Key_F1)
    until(lambda: glyphs(look, page) != before, "the options are up")
    look.press(Qt.Key.Key_Return)
    until(lambda: everyones(fake, "mangohud") is True, "the first option flips it for every game")
    assert own(fake, "mangohud") is None and len(told) == 1


def test_a_value_only_the_game_holds_offers_no_other_games(look):
    page = game_settings(look, "launch.runner")
    before = glyphs(look, page)
    look.press(Qt.Key.Key_Return)
    until(lambda: glyphs(look, page) != before, "the picker is up")
    assert "Y" not in glyphs(look, page), "the runner is the game's alone"
    look.press(Qt.Key.Key_Escape)


@pytest.fixture
def overlay(look, api):
    """The overlay the look's quick settings draw on over the game, the session stopped at the end."""
    layer = host.create_overlay(look.engine, look.window.size())
    api.home.attachOverlay(layer)
    layer.show()
    settle(layer)
    yield layer
    if api.universe.currentSession is not None:
        api.home.stop()
        until(lambda: api.universe.currentSession is None)
    layer.close()


def land(api, look, overlay, group, row):
    """Opens the quick settings over the running game on one row of a group: (panel, the scope line)."""
    api.home.openDock()
    overlay.requestActivate()
    until(overlay.isActive)
    panel = overlay.findChild(QObject, PANEL[look.name])
    until(lambda: panel.property("open") is True)
    if look.name == "reprise":
        panel.setProperty("index", [b["id"] for b in read(panel, "buttons")].index(group))
        QTest.keyClick(overlay, Qt.Key.Key_Return)
        until(lambda: panel.property("opened") is True)
        panel.setProperty("sub", [c["id"] for c in read(panel, "current")["children"]].index(row))
    else:
        panel.setProperty("icon", [i["id"] for i in read(panel, "icons")].index(group))
        panel.setProperty("zone", "panel")
        panel.setProperty("row", [r["id"] for r in read(panel, "panelRows")].index(row))
    scope = overlay.findChild(QObject, SCOPE[look.name])
    until(lambda: scope.property("visible") is True, "the panel says the setting is this game's and Y gives it to all")
    return panel, scope


@pytest.mark.parametrize("look", list(PANEL), indirect=True)
def test_a_quick_setting_is_the_games_and_y_gives_it_to_every_game(look, overlay, api, fake):
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    panel, scope = land(api, look, overlay, "perf", "fps")
    if where := os.environ.get("UNIVERSE_TEST_SHOTS"):
        until(lambda: panel.property("shown") is True and scope.property("opacity") == 1)
        settle(overlay)
        overlay.grabWindow().save(os.path.join(where, f"quick-settings-{look.name}.png"))
    QTest.keyClick(overlay, Qt.Key.Key_Right)
    picked = until(lambda: own(fake, "fps_limit", "mirrors-edge"), "a step is this game's")
    QTest.keyClick(overlay, Qt.Key.Key_F)
    until(lambda: everyones(fake, "fps_limit") == picked, "Y: every game's")
    assert own(fake, "fps_limit", "mirrors-edge") is None, "the game follows it"


@pytest.mark.parametrize("look", list(PANEL), indirect=True)
def test_a_decks_control_set_in_a_game_is_its_own_until_y_makes_it_the_machines(monkeypatch, look, overlay, api, fake):
    monkeypatch.setenv("UNIVERSE_DECK", "oled")
    api.system.reload()
    until(lambda: (api.system.control("tdp") or {}).get("value") == "15")

    def own_tdp():
        return (fake.game("mirrors-edge").get("system") or {}).get("tdp") or None

    def watts():
        return api.system.control("tdp")["value"]

    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game")
    land(api, look, overlay, "system", "sys_tdp")
    QTest.keyClick(overlay, Qt.Key.Key_Left)
    until(lambda: own_tdp() == "14", "a step is this game's")
    api.home.stop()
    until(lambda: api.universe.currentSession is None and watts() == "15", "its end puts the machine's own back")
    fake.launch("mirrors-edge", "")
    until(lambda: api.home.shown == "game" and watts() == "14", "its next launch brings the game's back")
    land(api, look, overlay, "system", "sys_tdp")
    QTest.keyClick(overlay, Qt.Key.Key_F)
    until(lambda: own_tdp() is None, "Y: the game lets its own go")
    api.home.stop()
    until(lambda: api.universe.currentSession is None)
    assert watts() == "14", "the machine's own now, kept after the game"
