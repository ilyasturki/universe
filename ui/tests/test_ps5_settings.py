import pytest
from looks import Look, call, invoke
from looks import read as value
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from uitest import pump, record, settle, until

from universe_ui.screens.controller import FakeWatcher


@pytest.fixture
def ps5(api):
    shown = Look(api, "ps5")
    warnings = []
    shown.engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    yield shown.window, shown.root, warnings
    seen = list(warnings)
    shown.close()
    assert seen == [], "no QML warning"


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)


def push(root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    return until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)


def glyphs(page):
    return [h["glyph"] for h in value(page, "hints")]


def sections(page):
    return [s["label"] for s in value(page, "sections")]


def focused_row(page):
    return next((value(rows, "currentRow") for rows in page.findChildren(QQuickItem, "settingsRows") if rows.property("cursorShown")), None)


def test_the_focused_row_wraps_its_whole_description(ps5):
    window, root, _warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {"section": "launch"})
    click(window, Qt.Key.Key_Right)
    rows = page.findChild(QQuickItem, "settingsRows")
    until(lambda: rows.property("cursorShown") is True)
    one_line = rows.property("rowHeight") + rows.property("detailLine")
    at = rows.property("index")
    assert value(rows, "currentRow")["key"] == "launch.gamescope"
    until(lambda: call(rows, "heightOf", at) > one_line, "the focused row's description runs on")
    click(window, Qt.Key.Key_Down)
    until(lambda: rows.property("index") != at)
    until(lambda: call(rows, "heightOf", at) == one_line, "left behind, it keeps one line")


def test_a_section_that_is_one_page_opens_it_straight_away(ps5):
    window, root, _warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {})
    roots = until(lambda: page.findChild(QQuickItem, "rootList"))
    roots.setProperty("index", [s["id"] for s in value(page, "sections")].index("controllers"))
    click(window, Qt.Key.Key_Return)
    top = until(lambda: (t := root.property("topPage")) is not page and t)
    assert top.property("activeFocus") is True and top.objectName() == "controllersPage"
    click(window, Qt.Key.Key_Escape)
    until(lambda: root.property("topPage") == page and page.property("level") == "root", "B lands back on the sections")


def test_the_sections_hold_still_while_the_cursor_moves_inside_them(ps5):
    window, root, _warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {})
    roots = until(lambda: page.findChild(QQuickItem, "rootList"))
    view = next(c for c in roots.childItems() if c.metaObject().className().startswith("QQuickListView"))
    rest = -view.property("topMargin")
    until(lambda: view.property("contentY") == rest, "the first row clear of the title, under the list's margin")
    for key in (Qt.Key.Key_Down, Qt.Key.Key_Up):
        at = roots.property("index")
        click(window, key)
        until(lambda at=at: roots.property("index") != at)
        pump(300)
        assert view.property("contentY") == rest, "a row already in view moves no list"


def test_settings_artwork_lists_every_game_and_fetches_the_missing_art(ps5):
    _window, root, _warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {"section": "artwork"})
    until(lambda: page.property("sectionId") == "artwork" and page.property("level") == "section")
    assert until(lambda: value(page, "content"))[0]["action"] == "artwork-fetch"
    until(lambda: [r for r in value(page, "content") if r.get("action") == "artwork-game"], "a row per game, the ones missing art first")


def test_a_runner_form_keeps_its_cards_when_y_shows_the_advanced_rows(ps5, api):
    window, root, _warnings = ps5
    form = api.screens.runner
    page = push(root, "pages/FormPage.qml", {"runner": "proton"})
    cards = until(lambda: sections(page))
    assert cards == [g["title"] for g in form.groups] and form.showAdvanced is False
    assert page.property("strip") is True, "a form is no screen of the console's: the hints strip shows"
    click(window, Qt.Key.Key_F)
    until(lambda: form.showAdvanced is True and sections(page) == cards, "Y: the cards stay")
    click(window, Qt.Key.Key_Right)
    click(window, Qt.Key.Key_Down, 3)
    row = until(lambda: (r := value(page, "currentRow")) and r["key"] == "gamescope" and r)
    assert row["origin"] == "default" and glyphs(page) == ["Y", "X", "B", "A"], "config.toml leaves gamescope to its default"
    click(window, Qt.Key.Key_Return)
    until(lambda: form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner")
    click(window, Qt.Key.Key_I)
    until(lambda: form.rows[row["form"]]["origin"] == "default", "X clears it back")
    click(window, Qt.Key.Key_Escape, 2)
    page = push(root, "pages/FormPage.qml", {"source": "gog"})
    until(lambda: sections(page) == [g["title"] for g in api.screens.source.groups] and glyphs(page) == ["Y", "B", "A"])


def test_a_module_form_toggles_the_module(ps5, api):
    window, root, _warnings = ps5
    page = push(root, "pages/FormPage.qml", {"module": "capture"})
    until(lambda: sections(page))
    click(window, Qt.Key.Key_Right)
    row = until(lambda: (r := value(page, "currentRow")) and r["key"] == "enabled" and r)
    assert row["value"] is True
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.module.rows[row["form"]]["value"] is False, "A on the switch turns the module off")
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.module.rows[row["form"]]["value"] is True)


@pytest.mark.parametrize(
    ("query", "section", "field", "want", "inside"),
    [("mangohud", "launch", "key", "launch.mangohud", True), ("switch", "themes", "theme", "switch2", False)],
    ids=["from-settings", "from-outside"],
)
def test_the_settings_search_lands_on_its_hit(ps5, api, query, section, field, want, inside):
    window, root, _warnings = ps5
    settings = push(root, "pages/SettingsPage.qml", {}) if inside else None
    push(root, "pages/SettingsSearchPage.qml", {})
    search = api.screens.search
    until(lambda: search.ready)
    search.query = query
    until(lambda: search.count > 0)
    click(window, Qt.Key.Key_F1)
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 1 and root.property("topPage").property("sectionId") == section, "the search went, Settings took the hit")
    top = root.property("topPage")
    assert settings in (None, top), "the Settings under the search took it"
    until(lambda: top.property("level") == "section" and top.property("zone") == "rows")
    until(lambda: (focused_row(top) or {}).get(field) == want)


def test_the_offered_walk_mutes_the_rows_and_enter_keeps_what_it_set_up_where_escape_undoes_it(ps5, api, fake):
    window, root, _warnings = ps5
    family = "8bitdo-pro-3"
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher(family)
    screen.start(watcher)
    until(lambda: root.property("modal") is True, "a family never set up: the walk is offered")
    click(window, Qt.Key.Key_Return)
    page = until(lambda: (top := root.property("topPage")) is not None and top.property("walking") is True and screen.walking and top)
    assert value(page, "hints") == [], "the walk's own line says what to press"
    assert focused_row(page) is None, "the pad is muted: the keys answer the walk, not the rows"

    def south():
        return next(s["codes"] for s in screen._families()[family]["slots"] if s["id"] == "south")

    # The core moves the code as the watcher learns it.
    def learn(code):
        fake._core.set_controller_button(family, "south", [code])
        watcher.emit({"event": "learned", "family": family, "slot": "south", "code": code, "from": None})

    learn("BTN_EAST")
    click(window, Qt.Key.Key_Return)
    until(lambda: not screen.walking and (focused_row(page) or {}).get("key") == "walk", "Enter ends the walk, back on the row it started from")
    assert south() == ["BTN_EAST"], "Enter keeps what the walk set up"
    click(window, Qt.Key.Key_Return)
    until(lambda: screen.walking)
    learn("BTN_WEST")
    click(window, Qt.Key.Key_Escape)
    until(lambda: not screen.walking and south() == ["BTN_EAST"], "Escape puts back what the walk changed")
    until(lambda: (focused_row(page) or {}).get("key") == "walk")


def test_a_launch_missing_its_runner_asks_to_install_then_plays(ps5, api, fake):
    window, root, _warnings = ps5
    game = fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    components = api.screens.components
    components.load()
    settle(components)
    fake.launchFailed.emit(game, f"{game}: RPCS3 not found (install it or set runners.rpcs3.exe)")
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True)
    finished = record(fake.jobFinished)
    click(window, Qt.Key.Key_Return)
    until(lambda: components.job is not None, "Install and Play is the default")
    until(lambda: finished)
    until(lambda: root.property("launching") is True, "in, the game starts again")


def test_the_components_tools_open_their_options(ps5, api):
    window, root, _warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {"section": "components"})
    settle(api.screens.components)
    until(lambda: page.property("sectionId") == "runners" and page.property("level") == "section")
    parts = until(lambda: (p := value(page, "parts")) and any(r.get("component") == "gamescope" for r in p[-1]["rows"]) and p)
    tools = parts[-1]["rows"]
    assert [r["component"] for r in tools] == ["gpu-screen-recorder", "gamescope"] and all(r["action"] == "component" for r in tools)
    page.setProperty("part", len(parts) - 1)
    click(window, Qt.Key.Key_Right)
    row = until(lambda: (r := focused_row(page)) and r.get("component") == "gpu-screen-recorder" and r)
    click(window, Qt.Key.Key_Return)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    assert len(value(popup, "items")) == len(api.screens.components.actions(row["component"]))


def test_a_doctor_check_an_install_fixes_asks_then_installs(ps5, api, fake):
    window, root, _warnings = ps5
    fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    page = push(root, "pages/SettingsPage.qml", {"section": "doctor"})
    settle(api.screens.components)
    row = until(lambda: next((r for r in value(page, "content") if r.get("component") == "rpcs3"), None))
    rows = value(page, "content")
    assert [r for r in rows if r.get("type") == "action"] == [row], "a check no install fixes stays a check"
    invoke(page, "activate", rows.index(row), row)
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True)
    assert "0.0.42-20069-3fa07db7" in dialog.property("message"), "the listing is in: the question names the build"
    finished = record(fake.jobFinished)
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.components.job is not None, "the first press installs")
    until(lambda: finished)


def test_the_controllers_page_names_an_unknown_button_and_b_leaves_the_test_on_its_row(ps5, api):
    window, root, _warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    until(lambda: screen.current)
    page = push(root, "pages/ControllersPage.qml", {})
    notice = page.findChild(QObject, "unknownButton")
    watcher.emit({"event": "unknown", "id": screen.current, "code": "BTN_TRIGGER_HAPPY1"})
    until(lambda: page.property("unknownCode") == "BTN_TRIGGER_HAPPY1" and notice.property("visible") is True)
    until(lambda: (focused_row(page) or {}).get("key") == "test")
    click(window, Qt.Key.Key_Return)
    until(lambda: screen.testing and page.property("testing") is True)
    click(window, Qt.Key.Key_Escape)
    until(lambda: not screen.testing and (focused_row(page) or {}).get("key") == "test", "B leaves the test on its row")
