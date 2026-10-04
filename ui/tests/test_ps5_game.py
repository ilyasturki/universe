import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtTest import QTest
from test_render import render

from conftest import record, until

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
    del engine
    assert mine == [], "no QML warning from the game pages"


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def push(root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    return until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)


def current_key(page):
    return (value(page, "currentRow") or {}).get("key")


def test_game_settings_land_a_hit_behind_advanced_and_edit_it(ps5, api, fake):
    window, root = ps5
    form = api.screens.gameSettings
    depth = root.property("depth")
    page = push(root, "pages/GameSettingsPage.qml", {"gameId": "the-technomancer", "key": "launch.ntsync"})
    assert root.property("depth") == depth + 1 and page.property("strip") is True
    until(lambda: page.property("zone") == "rows" and current_key(page) == "launch.ntsync", "the cursor on the hit")
    assert form.showAdvanced is True, "an advanced row: Advanced comes on"
    sections = [s["label"] for s in value(page, "sections")]
    assert sections[page.property("section")] == "Proton", "the hit's card"
    click(window, Qt.Key.Key_Return)
    until(
        lambda: fake.game("the-technomancer")["launch"]["ntsync"] is False and value(page, "currentRow")["origin"] == "game",
        "changing the value sets it on the game",
    )
    until(lambda: page.property("canReset") is True)
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    assert len(value(popup, "items")) == 4, "reset, the switch flipped for every Proton game, the game's value made theirs, advanced"
    click(window, Qt.Key.Key_Escape)
    until(lambda: popup.property("open") is False)
    click(window, Qt.Key.Key_I)
    until(lambda: "ntsync" not in fake.game("the-technomancer")["launch"], "X drops the game's own value")
    click(window, Qt.Key.Key_F)
    until(lambda: form.showAdvanced is False and [s["label"] for s in value(page, "sections")] == sections, "Y: the rows go, the cards stay")
    click(window, Qt.Key.Key_PageDown)
    until(lambda: sections[page.property("section")] == "Launch", "RT steps the card")
    click(window, Qt.Key.Key_Escape)
    until(lambda: page.property("zone") == "list")
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == depth, "B from the cards closes the page")


def test_the_trophies_list_the_games_achievements_and_sort_them(ps5, api):
    window, root = ps5
    store = api.screens.achievements
    page = push(root, "pages/AchievementsPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: store.count == 6 and len(value(page, "rows")) == 6)
    rows = value(page, "rows")
    assert page.property("strip") is False and page.property("progress") == 50
    assert [r["unlocked"] for r in rows] == [True, True, True, False, False, False], "earned first, as the store orders them"
    assert rows[-1]["key"] == "hidden" and rows[-1]["hidden"] == 1, "a hidden one keeps its secret"
    click(window, Qt.Key.Key_Down, 2)
    until(lambda: page.property("index") == 2)
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    labels = [i["label"] for i in value(popup, "items")]
    assert labels[:4] == ["Default Order", "Rarest First", "Most Common First", "Name (A–Z)"]
    click(window, Qt.Key.Key_Down)
    click(window, Qt.Key.Key_Return)
    until(lambda: page.property("order") == "rare")
    rows = value(page, "rows")
    rarities = [r["rarity"] for r in rows[:-1]]
    assert rarities == sorted(rarities) and rows[-1]["key"] == "hidden" and page.property("index") == 0, "the hidden ones stay last"
    click(window, Qt.Key.Key_I)
    until(lambda: not store.loading)
    assert store.count == 6, "X asks the store again"


def test_a_on_a_trophy_opens_its_whole_text_and_a_hidden_one_stays_shut(ps5, api):
    window, root = ps5
    store = api.screens.achievements
    page = push(root, "pages/AchievementsPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: store.count == 6 and len(value(page, "rows")) == 6)
    dialog = root.findChild(QObject, "dialog")
    trophy = value(page, "rows")[0]
    click(window, Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is True)
    assert dialog.property("message") == trophy["name"] and trophy["description"] in dialog.property("detail")
    click(window, Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is False and page.property("activeFocus"))
    click(window, Qt.Key.Key_End)
    until(lambda: page.property("index") == 5)
    click(window, Qt.Key.Key_Return)
    assert dialog.property("open") is False, "a hidden trophy keeps its secret"


def test_the_information_opens_a_screenshot_full_screen(ps5, api):
    window, root = ps5
    depth = root.property("depth")
    page = push(root, "pages/SoftwareInfoPage.qml", {"gameId": "batman-arkham-origins", "shot": 1})
    facts = [f["label"] for f in until(lambda: value(page, "facts"))]
    assert facts[:3] == ["Developer", "Publisher", "Release"] and "Runner" in facts and "Program" not in facts, "a store game: no program yet"
    until(lambda: page.property("viewerOpen") is True and page.property("zone") == "shots", "args.shot opens that screenshot")
    click(window, Qt.Key.Key_Left)
    click(window, Qt.Key.Key_Escape)
    until(lambda: page.property("viewerOpen") is False and page.property("shotIndex") == 0, "B closes the viewer on the shot it showed")
    assert root.property("depth") == depth + 1, "and only the viewer"
    click(window, Qt.Key.Key_Return)
    until(lambda: page.property("viewerOpen") is True)
    click(window, Qt.Key.Key_Return)
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    labels = [i["label"] for i in value(popup, "items")]
    assert labels[:3] == ["Play", "View Screenshots", "Trophies"]
    click(window, Qt.Key.Key_Down, 2)
    click(window, Qt.Key.Key_Return)
    until(
        lambda: root.property("depth") == depth + 2 and root.property("topPage").metaObject().className().startswith("AchievementsPage"),
        "Options reaches the trophies",
    )
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == depth + 1)
    page = push(root, "pages/SoftwareInfoPage.qml", {"gameId": "the-technomancer"})
    facts = {f["label"]: f["value"] for f in until(lambda: value(page, "facts"))}
    assert facts["Program"].endswith("TheTechnomancer.exe") and facts["Sessions"] == "23", "where it lives on disk, how often it was played"


def test_the_artwork_opens_a_slot_and_a_pick_puts_the_default_under_it(ps5, api, fake):
    window, root = ps5
    form = api.screens.artwork
    page = push(root, "pages/ArtworkPage.qml", {"gameId": "dead-cells"})
    depth = root.property("depth")
    until(lambda: form.gameId == "dead-cells" and [s["slot"] for s in value(page, "slots")][:2] == ["box_front", "square"])
    click(window, Qt.Key.Key_Right)
    until(lambda: page.property("index") == 1)
    click(window, Qt.Key.Key_Return)
    until(lambda: form.candidatesSlot == "square" and len(form.candidates) > 0)
    until(lambda: root.property("depth") == depth + 1 and root.property("topPage").property("slot") == "square")
    top = root.property("topPage")
    until(lambda: len(value(top, "cells")) == 2 + len(form.candidates))
    cells = value(top, "cells")
    assert cells[0]["kind"] == "now" and cells[1]["kind"] == "candidate" and cells[-1]["kind"] == "key"
    click(window, Qt.Key.Key_Right, 2)
    until(lambda: top.property("cellIndex") == 2)
    picked = record(fake.mediaChanged)
    click(window, Qt.Key.Key_Return)
    until(lambda: picked)
    until(lambda: value(top, "cells")[1]["kind"] == "under" and form.slot("square")["kind"] == "picked", "a pick puts the default under it")
    until(lambda: top.property("cellIndex") == 3, "the ring stays on the candidate that was picked")
    click(window, Qt.Key.Key_F1)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    assert "Back to Default" in [i["label"] for i in value(popup, "items")]
    click(window, Qt.Key.Key_Escape)


def test_the_play_log_lists_the_sessions_and_reads_one(ps5, api):
    window, root = ps5
    store = api.screens.sessions
    page = push(root, "pages/PlayLogPage.qml", {"gameId": "the-technomancer"})
    depth = root.property("depth")
    until(lambda: store.count > 0 and len(value(page, "entries")) == store.count)
    entries = value(page, "entries")
    assert page.property("strip") is False
    assert any(e["detail"].startswith("Crashed") for e in entries), "how a session ended"
    assert {e["icon"] for e in entries if e["detail"].startswith("Crashed")} == {"warning"}, "a crash wears a warning"
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == depth + 1)
    log = root.property("topPage")
    assert log.property("session") == entries[0]["session"] and log.property("strip") is True
    until(lambda: len(store.log) > 0 and not store.logLoading)
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == depth)
    page = push(root, "pages/PlayLogPage.qml", {"gameId": "batman-arkham-origins"})
    until(lambda: [e["key"] for e in value(page, "entries")] == ["none"], "never played: one still row says so")
    click(window, Qt.Key.Key_Return)
    assert root.property("topPage") == page, "and it opens nothing"
