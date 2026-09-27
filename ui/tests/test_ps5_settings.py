import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from test_onboarding import empty, empty_api, signed_out  # noqa: F401  (fixtures)
from test_render import render
from test_render import settle as settle_window

from conftest import pump, settle, wait_for
from universe_ui.screens.controller import FakeWatcher


@pytest.fixture
def ps5(api):
    api.theme.set("ps5")
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    warnings = []
    engine.warnings.connect(lambda ws: warnings.extend(w.toString() for w in ws))
    root = window.property("contentItem").childItems()[0].property("item")
    yield window, root, warnings
    window.close()
    pump(50)
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)
        pump(60)
    pump(80)


def push(window, root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    settle_window(window)
    pump(300)
    return root.property("topPage")


def labels(page):
    return [h["label"] for h in value(page, "hints")]


# The rows column holding the cursor: the SettingsRows with focus, its row under the cursor.
def focused_row(page):
    for item in page.findChildren(QQuickItem):
        if item.property("cursorShown") is True and item.property("currentRow") is not None:
            return value(item, "currentRow")
    return None


def test_a_runner_form_keeps_its_cards_when_y_shows_the_advanced_rows(ps5, api):
    window, root, warnings = ps5
    form = api.screens.runner
    page = push(window, root, "pages/FormPage.qml", {"runner": "proton"})
    sections = [s["label"] for s in value(page, "sections")]
    assert sections == ["Runner", "Proton", "Games"] and form.showAdvanced is False
    assert page.property("strip") is True, "a form is no screen of the console's: the hints strip shows"
    click(window, Qt.Key.Key_F)
    assert form.showAdvanced is True and [s["label"] for s in value(page, "sections")] == sections, "Y: the cards stay"
    click(window, Qt.Key.Key_Right)
    click(window, Qt.Key.Key_Down, 2)
    row = value(page, "currentRow")
    assert row["key"] == "gamescope" and row["origin"] == "global" and labels(page) == ["Hide advanced", "Reset", "Back", "Toggle"]
    click(window, Qt.Key.Key_Return)
    assert form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner"
    click(window, Qt.Key.Key_I)
    assert form.rows[row["form"]]["origin"] == "global", "X clears it back"
    click(window, Qt.Key.Key_Escape, 2)
    page = push(window, root, "pages/FormPage.qml", {"source": "gog"})
    pump(500)
    assert [s["label"] for s in value(page, "sections")] == ["Settings", "Sign-in"] and labels(page) == ["Show advanced", "Back", "OK"]
    assert warnings == []


def test_a_module_form_toggles_the_module(ps5, api):
    window, root, warnings = ps5
    page = push(window, root, "pages/FormPage.qml", {"module": "capture"})
    click(window, Qt.Key.Key_Right)
    row = value(page, "currentRow")
    assert row["key"] == "enabled" and row["value"] is True
    click(window, Qt.Key.Key_Return)
    pump(200)
    assert api.screens.module.rows[row["form"]]["value"] is False, "A on the switch turns the module off"
    click(window, Qt.Key.Key_Return)
    pump(200)
    assert api.screens.module.rows[row["form"]]["value"] is True
    assert warnings == []


@pytest.mark.parametrize(("query", "section", "field", "want"), [("mangohud", "launch", "key", "launch.mangohud"), ("switch", "themes", "theme", "switch2")])
def test_the_settings_search_lands_on_its_hit(ps5, api, query, section, field, want):
    window, root, warnings = ps5
    settings = push(window, root, "pages/SettingsPage.qml", {})
    push(window, root, "pages/SettingsSearchPage.qml", {})
    search = api.screens.search
    if not search.ready:
        wait_for(search.readyChanged, 5000)
    search.query = query
    pump(300)
    assert search.count > 0
    click(window, Qt.Key.Key_F1)
    click(window, Qt.Key.Key_Return)
    pump(400)
    assert root.property("depth") == 1 and root.property("topPage") == settings, "the search went, Settings took the hit"
    assert settings.property("sectionId") == section and settings.property("level") == "section" and settings.property("zone") == "rows"
    row = focused_row(settings)
    assert row is not None and row[field] == want
    assert warnings == []


def test_a_hit_from_outside_settings_brings_settings_up(ps5, api):
    window, root, warnings = ps5
    push(window, root, "pages/SettingsSearchPage.qml", {})
    search = api.screens.search
    if not search.ready:
        wait_for(search.readyChanged, 5000)
    search.query = "switch"
    pump(300)
    click(window, Qt.Key.Key_F1)
    click(window, Qt.Key.Key_Return)
    pump(500)
    top = root.property("topPage")
    assert root.property("depth") == 1 and top is not None and top.property("sectionId") == "themes"
    assert warnings == []


def test_the_walk_starts_from_the_offer(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("8bitdo-pro-3")
    screen.start(watcher)
    pump(200)
    assert root.property("modal") is True, "a family never set up: the walk is offered"
    click(window, Qt.Key.Key_Return)
    pump(300)
    page = root.property("topPage")
    assert page is not None and page.property("walking") is True and screen.walking
    assert value(page, "hints") == [], "the walk's own line says what to press"
    click(window, Qt.Key.Key_Escape)
    assert not screen.walking, "Esc stops the walk"
    assert warnings == []


def test_enter_keeps_what_the_walk_set_up_and_escape_undoes_it(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    messages = []
    screen.message.connect(messages.append)
    watcher = FakeWatcher("8bitdo-pro-3")
    screen.start(watcher)
    pump(200)
    click(window, Qt.Key.Key_Return)
    pump(300)
    page = root.property("topPage")
    assert screen.walking and focused_row(page) is None, "the pad is muted: the keys answer the walk, not the rows"
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": "south", "code": "BTN_EAST", "from": None})
    click(window, Qt.Key.Key_Return)
    assert not screen.walking and messages[-1] == "8BitDo Pro 3: 1 set up, the rest as they were"
    assert focused_row(page)["key"] == "walk", "back on the row it started from"
    click(window, Qt.Key.Key_Return)
    assert screen.walking
    click(window, Qt.Key.Key_Escape)
    assert not screen.walking and messages[-1] == "Setup canceled, nothing changed"
    assert focused_row(page)["key"] == "walk"
    assert warnings == []


def test_a_launch_missing_its_runner_asks_to_install_then_plays(ps5, api, fake):
    window, root, warnings = ps5
    game = fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    components = api.screens.components
    components.load()
    settle(components)
    fake.launchFailed.emit(game, f"{game}: RPCS3 not found (install it or set runners.rpcs3.exe)")
    pump(300)
    dialog = root.findChild(QObject, "dialog")
    assert dialog.property("open") is True and dialog.property("message") == "Install RPCS3 0.0.42-20069-3fa07db7 to play Demons Souls?"
    click(window, Qt.Key.Key_Return)
    assert components.job is not None, "Install and Play is the default"
    wait_for(fake.jobFinished, 5000)
    pump(300)
    assert root.property("launching") is True, "in, the game starts again"
    assert warnings == []


def test_settings_components_part_by_kind_and_a_opens_the_options(ps5, api):
    window, root, warnings = ps5
    page = push(window, root, "pages/SettingsPage.qml", {"section": "components"})
    settle(api.screens.components)
    pump(100)
    assert page.property("sectionId") == "components" and page.property("level") == "section"
    parts = {p["label"]: p["detail"] for p in value(page, "parts")}
    assert parts["Emulators"].endswith("installed") and "Proton" in parts and "No download" in parts
    click(window, Qt.Key.Key_Right)
    row = focused_row(page)
    form = api.screens.components
    assert row["action"] == "component" and form.rows[row["form"]]["label"] == row["label"]
    click(window, Qt.Key.Key_Return)
    popup = root.findChild(QObject, "popup")
    assert popup.property("open") is True
    assert [i["label"] for i in value(popup, "items")] == [a["label"] for a in form.actions(row["form"])]
    assert warnings == []


def test_a_doctor_check_an_install_fixes_asks_then_installs(ps5, api, fake):
    window, root, warnings = ps5
    fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    page = push(window, root, "pages/SettingsPage.qml", {"section": "doctor"})
    wait_for(api.screens.modules.doctorChanged, 3000)
    settle(api.screens.components)
    pump(100)
    rows = value(page, "content")
    row = next(r for r in rows if r.get("label") == "RPCS3")
    assert row["type"] == "action" and row["component"] == "rpcs3" and row["display"] == "Install"
    assert all(r.get("type") != "action" for r in rows if r.get("label") != "RPCS3"), "a check no install fixes stays a check"
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", rows.index(row)), Q_ARG("QVariant", row))
    pump(300)
    dialog = root.findChild(QObject, "dialog")
    assert dialog.property("open") is True and dialog.property("message") == "Install RPCS3 0.0.42-20069-3fa07db7?", (
        "the listing is in: the question names the build"
    )
    click(window, Qt.Key.Key_Return)
    assert api.screens.components.job is not None, "the first press installs"
    wait_for(fake.jobFinished, 5000)
    assert warnings == []


def test_an_unknown_button_is_named_on_the_page(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    pump(200)
    page = push(window, root, "pages/ControllersPage.qml", {})
    watcher.emit({"event": "unknown", "id": screen.current, "code": "BTN_TRIGGER_HAPPY1"})
    pump(50)
    shown = [t.property("text") for t in page.findChildren(QQuickItem) if t.inherits("QQuickText") and t.property("visible")]
    assert "BTN_TRIGGER_HAPPY1 is not one of the pad's buttons yet: learn it from a row" in shown
    assert warnings == []


def test_the_controllers_page_tests_the_buttons_full_width(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    screen.start(FakeWatcher("dualsense-edge"))
    pump(200)
    page = push(window, root, "pages/ControllersPage.qml", {})
    assert focused_row(page)["key"] == "test"
    click(window, Qt.Key.Key_Return)
    assert screen.testing and page.property("testing") is True
    click(window, Qt.Key.Key_Escape)
    assert not screen.testing and focused_row(page)["key"] == "test", "B leaves the test on its row"
    assert warnings == []


def test_the_wizard_opens_on_first_run_and_runs_again_from_about(empty_api, empty):  # noqa: F811
    def press(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)
            pump(150)

    def opened():
        top = root.property("topPage")
        return top is not None and top.property("form") is not None and top.property("last") is False

    signed_out(empty)
    empty_api.theme.set("ps5")
    empty_api.theme.takeLanding()
    engine, window = render(empty_api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    form = empty_api.screens.onboarding
    if form.busy:
        wait_for(form.busyChanged, 3000)
    settle_window(window)
    assert opened() and form.stepId == "found" and form.count == 6
    press(Qt.Key.Key_I)
    assert form.stepId == "stores", "X moves on"
    press(Qt.Key.Key_Escape)
    assert form.stepId == "found" and opened(), "B goes back a step, the setup stays"
    press(Qt.Key.Key_Down, 6)
    press(Qt.Key.Key_Return)
    assert form.stepId == "stores", "Down past the last row reaches the buttons, A on Continue moves on"
    press(Qt.Key.Key_Down, 3)
    press(Qt.Key.Key_Left)
    press(Qt.Key.Key_Return)
    assert form.stepId == "found", "the Back button goes back"
    press(Qt.Key.Key_Escape)
    settle_window(window)
    assert empty_api.memory.get("onboarded") is True and root.property("depth") == 0, "B on the first step skips the setup"
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "about"}))
    settle_window(window)
    pump(300)
    press(Qt.Key.Key_Down)
    press(Qt.Key.Key_Right)
    press(Qt.Key.Key_Return)
    settle_window(window)
    assert root.property("depth") == 2 and opened() and form.stepId == "found", "Settings › About runs it again"
    press(Qt.Key.Key_Return)
    wait_for(form.busyChanged, 3000)
    if form.busy:
        wait_for(form.busyChanged, 3000)
    settle_window(window)
    assert empty_api.allGames.count == 2, "A on the Lutris row imports behind the setup"
    press(Qt.Key.Key_Escape)
    settle_window(window)
    assert root.property("depth") == 1 and root.findChild(QObject, "homePage") is not None
    window.close()
    pump(50)
    del engine
