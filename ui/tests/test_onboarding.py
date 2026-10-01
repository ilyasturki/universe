import json

import pytest
from test_render import render

from conftest import index_of, record, rows_by_key, until


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


def signed_out(client):
    for source in client.core._data["sources"]:
        source["logged_in"] = False


def loaded(form):
    form.load()
    until(lambda: not form.busy)
    return form


def test_needed_once_on_an_empty_library(empty_api, empty):
    assert empty_api.screens.onboarding.needed is True
    empty_api.screens.onboarding.finish()
    assert empty.onboarded() is True and empty_api.screens.onboarding.needed is False, "the core keeps the flag"


def test_a_library_with_games_is_marked_so_an_emptied_one_never_asks(api, fake):
    assert api.screens.onboarding.needed is False
    assert fake.onboarded() is True


def test_the_ui_memory_flag_of_an_earlier_version_carries_over(empty_api, empty):
    empty_api.memory.set("onboarded", True)
    assert empty_api.screens.onboarding.needed is False and empty.onboarded() is True


def test_steps_and_found_rows(empty_api, empty):
    signed_out(empty)
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "stores", "preferences", "done"]
    assert form.stepId == "found"
    rows = rows_by_key(form)
    assert rows["lutris"]["display"] == "2 games" and rows["lutris"]["action"] == "Import" and rows["lutris"]["detail"] == "", "no title list"
    assert rows["heroic-gog"]["display"] == "1 game" and rows["heroic-gog"]["action"] == "Adopt"
    assert rows["steam"]["display"] == "3 games" and rows["steam"]["action"] == "Adopt" and rows["steam"]["via"] == "steam"
    assert rows["heroic-epic"]["display"] == "1 game" and rows["heroic-epic"]["action"] == "Adopt" and rows["heroic-epic"]["via"] == "epic"
    assert rows["heroic-amazon"]["display"] == "No games"
    assert rows["roms"]["display"] == "1 game" and rows["roms"]["action"] == "Import" and rows["roms"]["via"] == "roms"
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows] == ["logged_in", "link", "code", "enabled", "enabled"]
    assert form.rows[0]["display"] == "Sign in to install games" and [g["title"] for g in form.groups] == ["GOG", "Epic Games", "Steam"]
    assert (form.rows[3]["type"], form.rows[3]["label"]) == ("bool", "Use Epic Games"), "off by default, offered since Heroic is here"
    assert form.rows[4]["label"] == "Use Steam", "and Steam, whose folder is here"
    assert not any(r.get("quiet") for r in form.rows), "signed out, the sign-in rows show"
    form.back()
    assert form.stepId == "found"


def test_signed_in_stores_skip_their_step(empty_api, empty):
    empty.core._data["sources"] = [s for s in empty.core._data["sources"] if s["id"] == "gog"]
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "preferences", "done"], "every store signed in: nothing to do there"


def test_an_offered_store_turns_on_and_adopts_its_launchers_games_once_signed_in(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "stores", "preferences", "done"], "Epic is off, its launcher is here"
    finished = record(empty.jobFinished)
    assert form.runImport(index_of(form, "heroic-epic")) is True
    until(lambda: finished)
    assert empty.core._source("epic")["enabled"] is True, "adopting turns the source on"
    until(lambda: rows_by_key(form)["heroic-epic"]["display"] == "Sign in to Epic Games to adopt them")
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows if r["module"] == "epic"] == ["logged_in", "link", "code"], "on, it asks for a sign-in"
    empty.core._source("epic")["logged_in"] = True
    empty_api.screens.login._source = "epic"
    form._on_login(True, "Signed in.")
    until(lambda: len(finished) == 2)
    form.back()
    until(lambda: rows_by_key(form)["heroic-epic"]["display"] == "Nothing new", "signed in, the scan runs again")


def test_the_store_switch_turns_a_source_on_and_off(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    form.next()
    assert form.stepId == "stores"
    form.toggle(index_of(form, "enabled"))
    assert empty.core._source("epic")["enabled"] is True
    assert [r["key"] for r in form.rows if r["module"] == "epic"] == ["logged_in", "link", "code"], "its sign-in rows replace the switch"


def test_a_store_that_signs_in_with_an_api_key_is_worded_from_its_login_table(empty_api, empty):
    signed_out(empty)
    empty.core._source("steam")["enabled"] = True
    form = loaded(empty_api.screens.onboarding)
    form.next()
    kinds = {(r["module"], r["key"]): r["login"] for r in form.rows if r["key"] in ("link", "code")}
    assert kinds == {("gog", "link"): "code", ("gog", "code"): "code", ("steam", "link"): "key", ("steam", "code"): "key"}
    login = empty_api.screens.login
    login.begin("steam")
    assert login.status == empty.core._source("steam")["login"]["hint"], "the store's own hint under its link"


def test_found_rows_run_the_importers(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    assert form.runImport(index_of(form, "lutris")) is True
    until(lambda: not form.busy)
    assert rows_by_key(form)["lutris"]["display"] == "2 games added" and rows_by_key(form)["lutris"]["type"] == "static"
    assert empty_api.allGames.count == 2
    assert form.runImport(index_of(form, "lutris")) is False, "an import runs once"
    assert form.runImport(index_of(form, "heroic-amazon")) is False, "a launcher Universe cannot take over has nothing to run"
    finished = record(empty.jobFinished)
    assert form.runImport(index_of(form, "heroic-gog")) is True
    until(lambda: finished)
    until(lambda: rows_by_key(form)["heroic-gog"]["display"] == "Nothing new")
    assert empty.core.source_settings("gog")["scan_dirs"] == "/mnt/games/PC", "the other launcher's folder joined the source's scan_dirs"
    assert form.runImport(index_of(form, "roms")) is True
    until(lambda: not form.busy)
    assert rows_by_key(form)["roms"]["display"] == "1 game added"
    assert empty_api.allGames.count == 3 and empty.core.get("xenoblade-chronicles-3")["launch"]["runner"] == "eden"
    while form.stepId != "done":
        form.next()
    assert [(r["label"], r["display"]) for r in form.rows] == [("Lutris", "2 games added"), ("Emulator folders", "1 game added")]


def test_preferences_write_the_family_and_hdr(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    while form.stepId != "preferences":
        form.next()
    rows = rows_by_key(form)
    assert [g["title"] for g in form.groups] == ["Controller", "Graphics"] and form.groups[1]["meta"] == ""
    assert list(rows) == ["controller.family", "launch.hdr"], "the upscaler upgrades stay in Settings"
    assert rows["controller.family"]["display"] == "Xbox controller" and "Switch Pro Controller" in rows["controller.family"]["choices"]
    assert form.setValue(index_of(form, "controller.family"), "Switch Pro Controller") is True
    assert empty_api.screens.controller.family == "switch-pro" and empty_api.memory.get("controllerFamily") == "switch-pro"
    form.toggle(index_of(form, "launch.hdr"))
    assert empty.core.settings()["launch"]["hdr"] is True


@pytest.mark.parametrize(("config_owner", "owner"), [("home-manager", "home-manager"), ("", "config.toml")])
def test_read_only_config_skips_preferences(empty_api, empty, config_owner, owner):
    empty.core._config["config_writable"] = False
    empty.core._config["config_owner"] = config_owner
    signed_out(empty)
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "stores", "done"]
    assert form.runImport(index_of(form, "heroic-gog")) is True
    display = rows_by_key(form)["heroic-gog"]["display"]
    assert display.startswith("Settings are read-only: add /mnt/games/PC") and display.endswith(owner), "no scan_dirs write, so no scan"
    assert "scan_dirs" not in empty.core._config.get("sources", {}).get("gog", {})
    form.next()
    form.next()
    assert form.stepId == "done" and form.rows[-1]["key"] == "read_only" and owner in form.rows[-1]["detail"]


@pytest.mark.parametrize("theme", ["reprise", "switch2"])
def test_the_wizard_opens_on_first_run_in_both_looks(empty_api, empty, theme):
    from PySide6.QtCore import QObject, Qt
    from PySide6.QtTest import QTest

    def opened():
        return (
            root.property("subOpen") is True and root.property("subSource") == "pages/OnboardingPage.qml"
            if theme == "reprise"
            else root.property("depth") == 1 and root.property("topPage").property("last") is False
        )

    def press(key, times=1):
        for _ in range(times):
            QTest.keyClick(window, key)

    signed_out(empty)
    empty_api.theme.set(theme)
    empty_api.theme.takeLanding()
    _engine, window = render(empty_api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    home = until(lambda: root.property("activePage") if theme == "reprise" else root.findChild(QObject, "homePage"))
    form = empty_api.screens.onboarding
    until(lambda: opened() and not form.busy and form.stepId == "found" and form.count == 6)
    press(Qt.Key.Key_I)
    until(lambda: form.stepId == "stores", "X moves on")
    press(Qt.Key.Key_Escape)
    until(lambda: form.stepId == "found" and opened(), "B goes back a step, the dialog stays")
    press(Qt.Key.Key_Down, 6)
    press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "stores", "Down past the last row reaches the buttons, A on Continue moves on")
    press(Qt.Key.Key_Down, 5)
    press(Qt.Key.Key_Left)
    press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "found", "the Back button goes back")
    press(Qt.Key.Key_Escape)
    until(lambda: empty.onboarded() is True and not opened(), "B on the first step skips the setup")
    if theme == "reprise":
        press(Qt.Key.Key_Up)
    press(Qt.Key.Key_Right)
    until(lambda: home.property("onSetup") is True)
    press(Qt.Key.Key_Return)
    until(lambda: opened() and form.stepId == "found", "the empty Home's Set up entry runs it again")
    press(Qt.Key.Key_Return)
    until(lambda: empty_api.allGames.count == 2, "A on the Lutris row imports behind the dialog")
    press(Qt.Key.Key_Escape)
    until(lambda: not opened())
    if theme == "reprise":
        until(
            lambda: home.property("tileSelected") is False and (game := home.property("currentGame")) is not None and game.property("id") is not None,
            "the rail that filled behind the dialog lands on a game",
        )
        press(Qt.Key.Key_Down)
        press(Qt.Key.Key_Right, 2)
        until(lambda: home.property("tileSelected") is True, "Down leaves the hero pills for the rail, Right past the last game reaches the Library tile")
        press(Qt.Key.Key_Left)
        until(lambda: home.property("tileSelected") is False and home.property("currentGame") is not None)
    else:
        until(lambda: home.property("onSetup") is False and home.property("index") <= home.property("allIndex"))
    window.close()
