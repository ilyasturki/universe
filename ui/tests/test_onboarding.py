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


def test_steps_and_found_rows(empty_api):
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "stores", "preferences", "done"]
    assert form.stepId == "found"
    rows = rows_by_key(form)
    assert rows["lutris"]["display"] == "2 games" and rows["lutris"]["action"] == "Import" and rows["lutris"]["detail"] == "", "no title list"
    assert rows["heroic-gog"]["display"] == "1 game" and rows["heroic-gog"]["action"] == "Adopt"
    assert rows["steam"]["display"] == "3 games · not importable yet" and rows["steam"]["type"] == "static"
    assert rows["heroic-epic"]["display"] == "1 game · not importable yet" and rows["heroic-amazon"]["display"] == "No games"
    assert rows["roms"]["display"] == "1 game" and rows["roms"]["action"] == "Import" and rows["roms"]["via"] == "roms"
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows] == ["logged_in", "link", "code"]
    assert form.rows[0]["display"] == "Signed in as yasso" and form.groups[0]["title"] == "GOG"
    form.back()
    assert form.stepId == "found"


def test_found_rows_run_the_importers(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    assert form.runImport(index_of(form, "lutris")) is True
    wait_for(form.busyChanged, 3000)
    if form.busy:
        wait_for(form.busyChanged, 3000)
    assert rows_by_key(form)["lutris"]["display"] == "2 games added" and rows_by_key(form)["lutris"]["type"] == "static"
    assert empty_api.allGames.count == 2
    assert form.runImport(index_of(form, "lutris")) is False, "an import runs once"
    assert form.runImport(index_of(form, "steam")) is False, "a launcher Universe cannot take over has nothing to run"
    assert form.runImport(index_of(form, "heroic-gog")) is True
    wait_for(empty.jobFinished, 5000)
    pump(50)
    assert rows_by_key(form)["heroic-gog"]["display"] == "Nothing new"
    assert empty.core.source_settings("gog")["scan_dirs"] == "/mnt/games/PC", "the other launcher's folder joined the source's scan_dirs"
    assert form.runImport(index_of(form, "roms")) is True
    wait_for(form.busyChanged, 3000)
    if form.busy:
        wait_for(form.busyChanged, 3000)
    assert rows_by_key(form)["roms"]["display"] == "1 game added"
    assert empty_api.allGames.count == 3 and empty.core.get("xenoblade-chronicles-3")["launch"]["runner"] == "eden"
    while form.stepId != "done":
        form.next()
    assert [(r["label"], r["display"]) for r in form.rows] == [("Lutris", "2 games added"), ("Emulator folders", "1 game added")]


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
    assert [s["id"] for s in form.steps] == ["found", "stores", "done"]
    assert form.runImport(index_of(form, "heroic-gog")) is True
    assert rows_by_key(form)["heroic-gog"]["display"].startswith("Settings are read-only: add /mnt/games/PC"), "no scan_dirs write, so no scan"
    assert "scan_dirs" not in empty.core._config.get("sources", {}).get("gog", {})
    form.next()
    form.next()
    assert form.stepId == "done" and form.rows[-1]["key"] == "read_only" and "home-manager" in form.rows[-1]["detail"]


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
    assert opened() and form.stepId == "found" and form.count == 6
    press(Qt.Key.Key_I)
    assert form.stepId == "stores", "X moves on"
    press(Qt.Key.Key_Escape)
    assert form.stepId == "found" and opened(), "B goes back a step, the dialog stays"
    press(Qt.Key.Key_Escape)
    settle_window(window)
    assert empty_api.memory.get("onboarded") is True and not opened(), "B on the first step skips the setup"
    if theme == "reprise":
        root.openSetup()
    else:
        root.push("pages/OnboardingPage.qml", {})
    settle_window(window)
    assert opened() and form.stepId == "found", "the About row runs it again"
    window.close()
    pump(50)
