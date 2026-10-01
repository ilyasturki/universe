import pytest
from PySide6.QtCore import Q_ARG, QMetaObject, QObject, Qt
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from test_onboarding import empty, empty_api, signed_out  # noqa: F401  (fixtures)
from test_render import render

from conftest import record, settle, until
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
    del engine


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


def click(window, key, times=1):
    for _ in range(times):
        QTest.keyClick(window, key)


def push(root, source, args):
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", source), Q_ARG("QVariant", args))
    return until(lambda: (top := root.property("topPage")) is not None and top.property("activeFocus") and top)


def labels(page):
    return [h["label"] for h in value(page, "hints")]


def sections(page):
    return [s["label"] for s in value(page, "sections")]


# The rows column holding the cursor: the SettingsRows with focus, its row under the cursor.
def focused_row(page):
    for item in page.findChildren(QQuickItem):
        if item.property("cursorShown") is True and item.property("currentRow") is not None:
            return value(item, "currentRow")
    return None


def test_a_runner_form_keeps_its_cards_when_y_shows_the_advanced_rows(ps5, api):
    window, root, warnings = ps5
    form = api.screens.runner
    page = push(root, "pages/FormPage.qml", {"runner": "proton"})
    until(lambda: sections(page) == ["Runner", "Proton", "Games"])
    assert form.showAdvanced is False
    assert page.property("strip") is True, "a form is no screen of the console's: the hints strip shows"
    click(window, Qt.Key.Key_F)
    until(lambda: form.showAdvanced is True and sections(page) == ["Runner", "Proton", "Games"], "Y: the cards stay")
    click(window, Qt.Key.Key_Right)
    click(window, Qt.Key.Key_Down, 2)
    row = until(lambda: (r := value(page, "currentRow")) and r["key"] == "gamescope" and r)
    assert row["origin"] == "global" and labels(page) == ["Hide advanced", "Reset", "Back", "Toggle"]
    click(window, Qt.Key.Key_Return)
    until(lambda: form.rows[row["form"]]["origin"] == "runner", "toggling the inherited switch sets it on the runner")
    click(window, Qt.Key.Key_I)
    until(lambda: form.rows[row["form"]]["origin"] == "global", "X clears it back")
    click(window, Qt.Key.Key_Escape, 2)
    page = push(root, "pages/FormPage.qml", {"source": "gog"})
    until(lambda: sections(page) == ["Settings", "Sign-in"] and labels(page) == ["Show advanced", "Back", "OK"])
    assert warnings == []


def test_a_module_form_toggles_the_module(ps5, api):
    window, root, warnings = ps5
    page = push(root, "pages/FormPage.qml", {"module": "capture"})
    until(lambda: sections(page))
    click(window, Qt.Key.Key_Right)
    row = until(lambda: (r := value(page, "currentRow")) and r["key"] == "enabled" and r)
    assert row["value"] is True
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.module.rows[row["form"]]["value"] is False, "A on the switch turns the module off")
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.module.rows[row["form"]]["value"] is True)
    assert warnings == []


@pytest.mark.parametrize(("query", "section", "field", "want"), [("mangohud", "launch", "key", "launch.mangohud"), ("switch", "themes", "theme", "switch2")])
def test_the_settings_search_lands_on_its_hit(ps5, api, query, section, field, want):
    window, root, warnings = ps5
    settings = push(root, "pages/SettingsPage.qml", {})
    push(root, "pages/SettingsSearchPage.qml", {})
    search = api.screens.search
    until(lambda: search.ready)
    search.query = query
    until(lambda: search.count > 0)
    click(window, Qt.Key.Key_F1)
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 1 and root.property("topPage") == settings, "the search went, Settings took the hit")
    until(lambda: settings.property("sectionId") == section and settings.property("level") == "section" and settings.property("zone") == "rows")
    until(lambda: (focused_row(settings) or {}).get(field) == want)
    assert warnings == []


def test_a_hit_from_outside_settings_brings_settings_up(ps5, api):
    window, root, warnings = ps5
    push(root, "pages/SettingsSearchPage.qml", {})
    search = api.screens.search
    until(lambda: search.ready)
    search.query = "switch"
    until(lambda: search.count > 0)
    click(window, Qt.Key.Key_F1)
    click(window, Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 1 and (top := root.property("topPage")) is not None and top.property("sectionId") == "themes")
    assert warnings == []


def test_the_walk_starts_from_the_offer(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("8bitdo-pro-3")
    screen.start(watcher)
    until(lambda: root.property("modal") is True, "a family never set up: the walk is offered")
    click(window, Qt.Key.Key_Return)
    page = until(lambda: (top := root.property("topPage")) is not None and top.property("walking") is True and screen.walking and top)
    assert value(page, "hints") == [], "the walk's own line says what to press"
    click(window, Qt.Key.Key_Escape)
    until(lambda: not screen.walking, "Esc stops the walk")
    assert warnings == []


def test_enter_keeps_what_the_walk_set_up_and_escape_undoes_it(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    messages = []
    screen.message.connect(messages.append)
    watcher = FakeWatcher("8bitdo-pro-3")
    screen.start(watcher)
    until(lambda: root.property("modal") is True)
    click(window, Qt.Key.Key_Return)
    page = until(lambda: screen.walking and root.property("topPage"))
    assert focused_row(page) is None, "the pad is muted: the keys answer the walk, not the rows"
    watcher.emit({"event": "learned", "family": "8bitdo-pro-3", "slot": "south", "code": "BTN_EAST", "from": None})
    click(window, Qt.Key.Key_Return)
    until(lambda: not screen.walking and messages[-1] == "8BitDo Pro 3: 1 set up, the rest as they were")
    until(lambda: (focused_row(page) or {}).get("key") == "walk", "back on the row it started from")
    click(window, Qt.Key.Key_Return)
    until(lambda: screen.walking)
    click(window, Qt.Key.Key_Escape)
    until(lambda: not screen.walking and messages[-1] == "Setup canceled, nothing changed")
    until(lambda: (focused_row(page) or {}).get("key") == "walk")
    assert warnings == []


def test_a_launch_missing_its_runner_asks_to_install_then_plays(ps5, api, fake):
    window, root, warnings = ps5
    game = fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    components = api.screens.components
    components.load()
    settle(components)
    fake.launchFailed.emit(game, f"{game}: RPCS3 not found (install it or set runners.rpcs3.exe)")
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True)
    assert dialog.property("message") == "Install RPCS3 0.0.42-20069-3fa07db7 to play Demons Souls?"
    finished = record(fake.jobFinished)
    click(window, Qt.Key.Key_Return)
    until(lambda: components.job is not None, "Install and Play is the default")
    until(lambda: finished)
    until(lambda: root.property("launching") is True, "in, the game starts again")
    assert warnings == []


def test_settings_components_part_by_kind_and_a_opens_the_options(ps5, api):
    window, root, warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {"section": "components"})
    settle(api.screens.components)
    until(lambda: page.property("sectionId") == "components" and page.property("level") == "section")
    parts = until(lambda: {p["label"]: p["detail"] for p in value(page, "parts")})
    assert parts["Emulators"].endswith("installed") and "Proton" in parts and "No download" in parts
    click(window, Qt.Key.Key_Right)
    row = until(lambda: focused_row(page))
    form = api.screens.components
    assert row["action"] == "component" and form.rows[row["form"]]["label"] == row["label"]
    click(window, Qt.Key.Key_Return)
    popup = root.findChild(QObject, "popup")
    until(lambda: popup.property("open") is True)
    assert [i["label"] for i in value(popup, "items")] == [a["label"] for a in form.actions(row["form"])]
    assert warnings == []


def test_the_component_bar_hides_by_x_or_its_cross_and_uninstall_asks_in_red(ps5, api, fake):
    from PySide6.QtCore import QPointF

    from universe_ui import gamepad

    window, root, warnings = ps5
    page = push(root, "pages/SettingsPage.qml", {"section": "components"})
    form = api.screens.components
    settle(form)
    until(lambda: page.property("sectionId") == "components" and page.property("level") == "section")
    finished = record(fake.jobFinished)

    def row_of(ident):
        return next(i for i, r in enumerate(form.rows) if r.get("component") == ident and r["key"] == "component")

    def hidden():
        return form.job is None and page.property("componentsBar") is False

    assert form.act(row_of("wine"), "install")
    until(lambda: {"glyph": "X", "label": "Hide progress"} in value(page, "hints"))
    click(window, Qt.Key.Key_I)  # X
    until(hidden, "X hides it")
    until(lambda: len(finished) == 1)
    assert form.act(row_of("rpcs3"), "install")
    closer = page.findChild(QQuickItem, "hideJob")
    until(closer.isVisible)
    p = closer.mapToScene(QPointF(closer.width() / 2, closer.height() / 2))
    gamepad.touch(window, [(p.x(), p.y())], 0)
    until(hidden, "so does a tap on its ×")
    until(lambda: len(finished) == 2)
    QMetaObject.invokeMethod(page, "componentAction", Q_ARG("QVariant", row_of("xemu")), Q_ARG("QVariant", "uninstall"))
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True)
    assert dialog.property("message") == "Uninstall xemu?" and dialog.property("dangerIndex") == 1, "Uninstall is the red button"
    assert warnings == []


def test_a_doctor_check_an_install_fixes_asks_then_installs(ps5, api, fake):
    window, root, warnings = ps5
    fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    page = push(root, "pages/SettingsPage.qml", {"section": "doctor"})
    settle(api.screens.components)
    row = until(lambda: next((r for r in value(page, "content") if r.get("label") == "RPCS3" and r.get("type") == "action"), None))
    rows = value(page, "content")
    assert row["component"] == "rpcs3" and row["display"] == "Install"
    assert all(r.get("type") != "action" for r in rows if r.get("label") != "RPCS3"), "a check no install fixes stays a check"
    QMetaObject.invokeMethod(page, "activate", Q_ARG("QVariant", rows.index(row)), Q_ARG("QVariant", row))
    dialog = root.findChild(QObject, "dialog")
    until(lambda: dialog.property("open") is True)
    assert dialog.property("message") == "Install RPCS3 0.0.42-20069-3fa07db7?", "the listing is in: the question names the build"
    finished = record(fake.jobFinished)
    click(window, Qt.Key.Key_Return)
    until(lambda: api.screens.components.job is not None, "the first press installs")
    until(lambda: finished)
    assert warnings == []


def test_an_unknown_button_is_named_on_the_page(ps5, api):
    _window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    watcher = FakeWatcher("dualsense-edge")
    screen.start(watcher)
    until(lambda: screen.current)
    page = push(root, "pages/ControllersPage.qml", {})
    watcher.emit({"event": "unknown", "id": screen.current, "code": "BTN_TRIGGER_HAPPY1"})

    def shown():
        return [t.property("text") for t in page.findChildren(QQuickItem) if t.inherits("QQuickText") and t.property("visible")]

    until(lambda: "BTN_TRIGGER_HAPPY1 is not one of the pad's buttons yet: learn it from a row" in shown())
    assert warnings == []


def test_the_controllers_page_tests_the_buttons_full_width(ps5, api):
    window, root, warnings = ps5
    screen = api.screens.controller
    screen.restart_ms = 0
    screen.start(FakeWatcher("dualsense-edge"))
    until(lambda: screen.current)
    page = push(root, "pages/ControllersPage.qml", {})
    until(lambda: (focused_row(page) or {}).get("key") == "test")
    click(window, Qt.Key.Key_Return)
    until(lambda: screen.testing and page.property("testing") is True)
    click(window, Qt.Key.Key_Escape)
    until(lambda: not screen.testing and (focused_row(page) or {}).get("key") == "test", "B leaves the test on its row")
    assert warnings == []


def test_the_wizard_opens_on_first_run_and_runs_again_from_about(empty_api, empty):  # noqa: F811
    def press(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    def opened():
        top = root.property("topPage")
        return top is not None and top.property("form") is not None and top.property("last") is False

    signed_out(empty)
    empty_api.theme.set("ps5")
    empty_api.theme.takeLanding()
    engine, window = render(empty_api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    form = empty_api.screens.onboarding
    until(lambda: opened() and not form.busy and form.stepId == "found" and form.count == 6)
    press(Qt.Key.Key_I)
    until(lambda: form.stepId == "stores", "X moves on")
    press(Qt.Key.Key_Escape)
    until(lambda: form.stepId == "found" and opened(), "B goes back a step, the setup stays")
    press(Qt.Key.Key_Down, 6)
    press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "stores", "Down past the last row reaches the buttons, A on Continue moves on")
    press(Qt.Key.Key_Down, 5)
    press(Qt.Key.Key_Left)
    press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "found", "the Back button goes back")
    press(Qt.Key.Key_Escape)
    until(lambda: empty_api.memory.get("onboarded") is True and root.property("depth") == 0, "B on the first step skips the setup")
    QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "about"}))
    until(lambda: (top := root.property("topPage")) is not None and top.property("sectionId") == "about" and top.property("activeFocus"))
    press(Qt.Key.Key_Down)
    press(Qt.Key.Key_Right)
    press(Qt.Key.Key_Return)
    until(lambda: root.property("depth") == 2 and opened() and form.stepId == "found", "Settings › About runs it again")
    press(Qt.Key.Key_Return)
    until(lambda: empty_api.allGames.count == 2, "A on the Lutris row imports behind the setup")
    press(Qt.Key.Key_Escape)
    until(lambda: root.property("depth") == 1 and root.findChild(QObject, "homePage") is not None)
    window.close()
    del engine
