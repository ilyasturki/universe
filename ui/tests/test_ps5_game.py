import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_render import render, settle

from conftest import pump, wait_for

PAGES = (
    "AchievementsPage",
    "SoftwareInfoPage",
    "GameSettingsPage",
    "ArtworkPage",
    "ArtworkSlotPage",
    "PlayLogPage",
    "SessionLogPage",
    "GameCellGrid",
    "GameShotViewer",
    "TrophyCard",
)


@pytest.fixture
def ps5(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root
    mine = [w for w in warnings if any(p + ".qml" in w for p in PAGES)]
    window.close()
    pump(50)
    del engine
    assert mine == [], "no QML warning from the game pages"


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def push(window, root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    settle(window)
    pump(300)
    return root.property("topPage")


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)
    pump(80)


def until(test, signal, timeout_ms=3000):
    if not test():
        wait_for(signal, timeout_ms)
        pump(100)
    assert test()


def test_game_settings_land_a_hit_behind_advanced_and_edit_it(ps5, api, fake):
    window, root = ps5
    form = api.screens.gameSettings
    depth = root.property("depth")
    page = push(window, root, "pages/GameSettingsPage.qml", {"gameId": "the-technomancer", "key": "launch.ntsync"})
    assert root.property("depth") == depth + 1 and page.property("strip") is True
    assert form.showAdvanced is True, "an advanced row: Advanced comes on"
    sections = [s["label"] for s in value(page, "sections")]
    assert sections[page.property("section")] == "Proton", "the hit's card"
    assert page.property("zone") == "rows" and value(page, "currentRow")["key"] == "launch.ntsync", "the cursor on the hit"
    click(window, Qt.Key.Key_Return)
    assert fake.game("the-technomancer")["launch"]["ntsync"] is False and value(page, "currentRow")["origin"] == "game", (
        "changing the value sets it on the game"
    )
    assert page.property("canReset") is True
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    assert popup.property("open") is True and [i["label"] for i in value(popup, "items")] == ["Reset to Default", "Hide Advanced Settings"]
    click(window, Qt.Key.Key_Escape)
    assert popup.property("open") is False
    click(window, Qt.Key.Key_I)
    assert "ntsync" not in fake.game("the-technomancer")["launch"], "X drops the game's own value"
    click(window, Qt.Key.Key_F)
    assert form.showAdvanced is False and [s["label"] for s in value(page, "sections")] == sections, "Y: the rows go, the cards stay"
    click(window, Qt.Key.Key_PageDown)
    assert sections[page.property("section")] == "Launch", "RT steps the card"
    click(window, Qt.Key.Key_Escape)
    assert page.property("zone") == "list"
    click(window, Qt.Key.Key_Escape)
    assert root.property("depth") == depth, "B from the cards closes the page"


def test_the_trophies_list_the_games_achievements_and_sort_them(ps5, api):
    window, root = ps5
    store = api.screens.achievements
    page = push(window, root, "pages/AchievementsPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: store.count == 6, store.rowsChanged)
    rows = value(page, "rows")
    assert page.property("strip") is False and page.property("progress") == 50
    assert [r["unlocked"] for r in rows] == [True, True, True, False, False, False], "earned first, as the store orders them"
    assert rows[-1]["masked"] is True and rows[-1]["name"] == "Hidden achievement", "a hidden one keeps its secret"
    click(window, Qt.Key.Key_Down, 2)
    assert page.property("index") == 2 and value(page, "current")["name"] == rows[2]["name"], "the lit one is told in full"
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    labels = [i["label"] for i in value(popup, "items")]
    assert popup.property("open") is True and labels[:4] == ["Default Order", "Rarest First", "Most Common First", "Name (A–Z)"]
    click(window, Qt.Key.Key_Down)
    click(window, Qt.Key.Key_Return)
    rarities = [r["rarity"] for r in value(page, "rows")]
    assert page.property("order") == "rare" and rarities == sorted(rarities) and page.property("index") == 0
    click(window, Qt.Key.Key_I)
    until(lambda: not store.loading, store.stateChanged)
    assert store.count == 6, "X asks the store again"


def test_the_information_opens_a_screenshot_full_screen(ps5, api):
    window, root = ps5
    depth = root.property("depth")
    page = push(window, root, "pages/SoftwareInfoPage.qml", {"gameId": "batman-arkham-origins", "shot": 1})
    facts = [f["label"] for f in value(page, "facts")]
    assert facts[:3] == ["Developer", "Publisher", "Release"] and "Runner" in facts and "Program" not in facts, "a store game: no program yet"
    assert page.property("viewerOpen") is True and page.property("zone") == "shots", "args.shot opens that screenshot"
    click(window, Qt.Key.Key_Left)
    click(window, Qt.Key.Key_Escape)
    assert page.property("viewerOpen") is False and page.property("shotIndex") == 0, "B closes the viewer on the shot it showed"
    assert root.property("depth") == depth + 1, "and only the viewer"
    click(window, Qt.Key.Key_Return)
    assert page.property("viewerOpen") is True
    click(window, Qt.Key.Key_Return)
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    labels = [i["label"] for i in value(popup, "items")]
    assert popup.property("open") is True and labels[:3] == ["Play", "View Screenshots", "Trophies"]
    click(window, Qt.Key.Key_Down, 2)
    click(window, Qt.Key.Key_Return)
    top = root.property("topPage")
    assert root.property("depth") == depth + 2 and top.metaObject().className().startswith("AchievementsPage"), "Options reaches the trophies"
    click(window, Qt.Key.Key_Escape)
    page = push(window, root, "pages/SoftwareInfoPage.qml", {"gameId": "the-technomancer"})
    facts = {f["label"]: f["value"] for f in value(page, "facts")}
    assert facts["Program"].endswith("TheTechnomancer.exe") and facts["Sessions"] == "23", "where it lives on disk, how often it was played"


def test_the_artwork_opens_a_slot_and_a_pick_puts_the_default_under_it(ps5, api, fake):
    window, root = ps5
    form = api.screens.artwork
    page = push(window, root, "pages/ArtworkPage.qml", {"gameId": "dead-cells"})
    depth = root.property("depth")
    assert form.gameId == "dead-cells" and [s["slot"] for s in value(page, "slots")][:2] == ["box_front", "square"]
    click(window, Qt.Key.Key_Right)
    assert page.property("index") == 1
    click(window, Qt.Key.Key_Return)
    settle(window)
    until(lambda: form.candidatesSlot == "square" and len(form.candidates) > 0, form.candidatesChanged)
    top = root.property("topPage")
    assert root.property("depth") == depth + 1 and top.property("slot") == "square"
    cells = value(top, "cells")
    assert cells[0]["kind"] == "now" and cells[1]["kind"] == "candidate" and len(cells) == 1 + len(form.candidates)
    click(window, Qt.Key.Key_Right, 2)
    assert top.property("cellIndex") == 2
    click(window, Qt.Key.Key_Return)
    wait_for(fake.mediaChanged, 3000)
    pump(200)
    cells = value(top, "cells")
    assert cells[1]["kind"] == "under" and form.slot("square")["kind"] == "picked", "a pick puts the default under it"
    assert top.property("cellIndex") == 3, "the ring stays on the candidate that was picked"
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    assert popup.property("open") is True and "Back to Default" in [i["label"] for i in value(popup, "items")]
    click(window, Qt.Key.Key_Escape)


def test_the_play_log_lists_the_sessions_and_reads_one(ps5, api):
    window, root = ps5
    store = api.screens.sessions
    page = push(window, root, "pages/PlayLogPage.qml", {"gameId": "the-technomancer"})
    depth = root.property("depth")
    until(lambda: store.count > 0, store.rowsChanged)
    entries = value(page, "entries")
    assert len(entries) == store.count and page.property("strip") is False
    assert any(e["detail"].startswith("Crashed") for e in entries), "how a session ended"
    click(window, Qt.Key.Key_Return)
    log = root.property("topPage")
    assert root.property("depth") == depth + 1 and log.property("session") == entries[0]["session"] and log.property("strip") is True
    until(lambda: len(store.log) > 0 and not store.logLoading, store.logChanged)
    click(window, Qt.Key.Key_Escape)
    assert root.property("depth") == depth
    page = push(window, root, "pages/PlayLogPage.qml", {"gameId": "batman-arkham-origins"})
    pump(200)
    entries = value(page, "entries")
    assert [e["key"] for e in entries] == ["none"], "never played: one still row says so"
    click(window, Qt.Key.Key_Return)
    assert root.property("topPage") == page, "and it opens nothing"
