import json

import pytest
from test_render import render
from test_render import settle as settle_window

from conftest import index_of, pump, rows_by_key, wait_for


@pytest.fixture
def empty(app, xdg, tmp_path):
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    data = json.loads(FIXTURE.read_text())
    data["games"] = []
    fixture = tmp_path / "empty.json"
    fixture.write_text(json.dumps(data))
    client = CoreClient(FakeCore(fixture, tmp_path / "core"))
    yield client
    client.shutdown()


@pytest.fixture
def empty_api(empty, tmp_path):
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    api = Api(empty, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    yield api
    api.shutdown()


def loaded(form):
    form.load()
    wait_for(form.busyChanged, 3000)
    if form.busy:
        wait_for(form.busyChanged, 3000)
    return form


def test_needed_once_on_an_empty_library(api, empty_api):
    assert empty_api.screens.onboarding.needed is True
    assert api.screens.onboarding.needed is False, "a library with games never sees the wizard"
    assert api.memory.get("onboarded") is True, "and is marked so an emptied library does not bring it up later"
    empty_api.screens.onboarding.finish()
    assert empty_api.memory.get("onboarded") is True and empty_api.screens.onboarding.needed is False


def test_steps_and_discover_rows(empty_api):
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["discover", "stores", "import", "preferences", "done"]
    assert form.stepId == "discover"
    rows = rows_by_key(form)
    assert rows["lutris"]["display"] == "2 games" and rows["lutris"]["detail"] == "Celeste, Hades"
    assert rows["steam"]["display"] == "3 games · not importable yet" and rows["steam"]["detail"].startswith("Steam games launch through Steam")
    assert rows["heroic-amazon"]["display"] == "No games"
    assert all(r["type"] == "static" for r in form.rows)
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows] == ["logged_in", "link", "code"]
    assert form.rows[0]["display"] == "Signed in as yasso" and form.groups[0]["title"] == "GOG"
    form.back()
    assert form.stepId == "discover"


def test_import_step_runs_the_importers(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    form.next()
    form.next()
    assert form.stepId == "import"
    rows = rows_by_key(form)
    assert rows["lutris"]["action"] == "Import" and rows["lutris"]["display"] == "2 games to import"
    assert rows["heroic-gog"]["action"] == "Adopt" and rows["heroic-gog"]["display"] == "1 game to adopt"
    assert form.runImport(0) is True
    wait_for(form.busyChanged, 3000)
    if form.busy:
        wait_for(form.busyChanged, 3000)
    assert rows_by_key(form)["lutris"]["display"] == "2 games added" and rows_by_key(form)["lutris"]["action"] == ""
    assert empty_api.allGames.count == 2
    assert form.runImport(0) is False, "an import runs once"
    assert form.runImport(1) is True
    wait_for(empty.jobFinished, 5000)
    pump(50)
    assert rows_by_key(form)["heroic-gog"]["display"] == "Nothing new"
    assert empty.core.source_settings("gog")["scan_dirs"] == "/mnt/games/PC", "the other launcher's folder joined the source's scan_dirs"
    form.next()
    form.next()
    assert form.stepId == "done" and form.rows[0]["display"] == "2 games from Lutris"


def test_preferences_write_the_family_and_the_upgrades(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    while form.stepId != "preferences":
        form.next()
    rows = rows_by_key(form)
    assert [g["title"] for g in form.groups] == ["Controller", "Graphics"]
    assert rows["controller.family"]["display"] == "Xbox controller" and "Switch Pro Controller" in rows["controller.family"]["choices"]
    assert rows["launch.fsr4_upgrade"]["choices"] == ["auto", "on", "off"], "no `default` entry: off is the default"
    assert "launch.dlss_upgrade" not in rows, "an upgrade that does not fit the GPU is not offered"
    assert form.setValue(index_of(form, "controller.family"), "Switch Pro Controller") is True
    assert empty_api.screens.controller.family == "switch-pro" and empty_api.memory.get("controllerFamily") == "switch-pro"
    assert form.setValue(index_of(form, "launch.fsr4_upgrade"), "auto") is True
    assert empty.core.settings()["launch"]["fsr4_upgrade"] == "auto"
    form.toggle(index_of(form, "launch.hdr"))
    assert empty.core.settings()["launch"]["hdr"] is True


def test_read_only_config_skips_preferences(empty_api, empty):
    empty.core._config["config_writable"] = False
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["discover", "stores", "import", "done"]
    while form.stepId != "import":
        form.next()
    assert form.runImport(index_of(form, "heroic-gog")) is True
    assert rows_by_key(form)["heroic-gog"]["display"].startswith("Settings are read-only: add /mnt/games/PC"), "no scan_dirs write, so no scan"
    assert "scan_dirs" not in empty.core._config.get("sources", {}).get("gog", {})
    form.next()
    assert form.rows[1]["key"] == "read_only" and "home-manager" in form.rows[1]["detail"]


@pytest.mark.parametrize("theme", ["reprise", "switch2"])
def test_the_wizard_opens_on_first_run_in_both_looks(empty_api, theme):
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    def opened():
        return (
            root.property("subOpen") is True and root.property("subSource") == "pages/OnboardingPage.qml"
            if theme == "reprise"
            else root.property("depth") == 1 and root.property("topPage").property("last") is False
        )

    def press(key):
        QTest.keyClick(window, key)
        pump(150)

    empty_api.theme.set(theme)
    empty_api.theme.takeLanding()
    _engine, window = render(empty_api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    form = empty_api.screens.onboarding
    if form.busy:
        wait_for(form.busyChanged, 3000)
    settle_window(window)
    assert opened() and form.stepId == "discover" and form.count == 5
    press(Qt.Key.Key_I)
    assert form.stepId == "stores", "X moves on"
    press(Qt.Key.Key_Escape)
    assert form.stepId == "discover" and opened(), "B goes back a step, the page stays"
    press(Qt.Key.Key_Escape)
    settle_window(window)
    assert empty_api.memory.get("onboarded") is True and not opened(), "B on the first step skips the setup"
    if theme == "reprise":
        root.openSetup()
    else:
        root.push("pages/OnboardingPage.qml", {})
    settle_window(window)
    assert opened() and form.stepId == "discover", "the About row runs it again"
    window.close()
    pump(50)
