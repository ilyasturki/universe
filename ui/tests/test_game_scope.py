import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_render import render

from conftest import record, until

LOOKS = ["reprise", "ps5", "switch2"]


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
