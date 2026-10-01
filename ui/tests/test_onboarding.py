import json
import threading

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
    assert {key: (row["type"], row["via"]) for key, row in rows.items()} == {
        "lutris": ("action", "lutris"),
        "steam": ("action", "steam"),
        "heroic-gog": ("action", "gog"),
        "heroic-epic": ("action", "epic"),
        "roms": ("action", "roms"),
    }, "Heroic's Amazon games, with nothing to bring over, stay out"
    assert form.idle is False
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows] == ["logged_in", "link", "code", "enabled", "enabled"]
    assert [g["title"] for g in form.groups] == ["GOG", "Epic Games", "Steam"]
    assert [(r["type"], r["module"]) for r in form.rows[3:]] == [("bool", "epic"), ("bool", "steam")], "off, offered since their launchers are here"
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
    until(lambda: rows_by_key(form)["heroic-epic"]["state"] == "waiting", "nothing found while signed out: it waits for the sign-in")
    form.next()
    assert form.stepId == "stores" and [r["key"] for r in form.rows if r["module"] == "epic"] == ["logged_in", "link", "code"], "on, it asks for a sign-in"
    empty.core._source("epic")["logged_in"] = True
    empty_api.screens.login._source = "epic"
    form._on_login(True, "Signed in.")
    until(lambda: len(finished) == 2)
    form.back()
    until(lambda: rows_by_key(form)["heroic-epic"]["state"] == "imported", "signed in, the scan runs again")


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
    lutris = index_of(form, "lutris")
    assert form.runImport(lutris) is True
    until(lambda: rows_by_key(form)["lutris"]["state"] == "imported")
    assert (rows_by_key(form)["lutris"]["type"], rows_by_key(form)["lutris"]["count"]) == ("static", 2)
    assert empty_api.allGames.count == 2
    assert form.runImport(lutris) is False, "an import runs once"
    finished = record(empty.jobFinished)
    assert form.runImport(index_of(form, "heroic-gog")) is True
    until(lambda: finished)
    until(lambda: rows_by_key(form)["heroic-gog"]["state"] == "imported" and rows_by_key(form)["heroic-gog"]["count"] == 0)
    assert empty.core.source_settings("gog")["scan_dirs"] == "/mnt/games/PC", "the other launcher's folder joined the source's scan_dirs"
    assert form.runImport(index_of(form, "roms")) is True
    until(lambda: rows_by_key(form)["roms"]["state"] == "imported")
    assert empty_api.allGames.count == 3 and empty.core.get("xenoblade-chronicles-3")["launch"]["runner"] == "eden"
    while form.stepId != "done":
        form.next()
    assert [(r["key"], r["state"]) for r in form.rows] == [("lutris", "imported"), ("roms", "imported")], "what came in, and nothing for an empty adoption"


def test_the_load_asks_the_core_off_the_ui_thread(empty_api, empty):
    calls = []

    def spied(name, real):
        def spy(*args):
            calls.append((name, threading.current_thread() is threading.main_thread()))
            return real(*args)

        return spy

    for name in ("discover", "sources", "settings", "gpu", "launch_keys"):
        setattr(empty.core, name, spied(name, getattr(empty.core, name)))
    form = loaded(empty_api.screens.onboarding)
    assert {"discover", "sources", "settings"} <= {name for name, _ in calls}
    assert [name for name, main in calls if main] == [], "the first sources() asks every store over the network"
    while form.stepId != "done":
        form.next()
    assert [name for name, main in calls if main and name in ("discover", "sources")] == [], "nor does a step"


def test_an_import_still_running_leaves_the_rest_of_the_setup_usable(empty_api, empty):
    gate, real = threading.Event(), empty.core.import_lutris
    empty.core.import_lutris = lambda apply: (gate.wait(10), real(apply))[1]
    form = loaded(empty_api.screens.onboarding)
    assert form.runImport(index_of(form, "lutris")) is True
    assert rows_by_key(form)["lutris"]["state"] == "importing"
    assert form.runImport(index_of(form, "roms")) is True and rows_by_key(form)["roms"]["state"] == "queued", "the other importer waits its turn"
    assert form.runImport(index_of(form, "heroic-gog")) is True, "an adoption runs beside it"
    while form.stepId != "preferences":
        form.next()
    form.toggle(index_of(form, "launch.hdr"))
    assert empty.core.settings()["launch"]["hdr"] is True, "a preference is written while the import runs"
    form.next()
    states = {r["key"]: r["state"] for r in form.rows}
    assert form.stepId == "done" and (states["lutris"], states["roms"]) == ("importing", "queued"), "the summary says so"
    gate.set()
    until(lambda: {r["key"]: r["state"] for r in form.rows} == {"lutris": "imported", "roms": "imported"}, "and follows them to their end")
    until(lambda: any(job["kind"] == "media" for job in empty.jobs()), "the emulator games' art comes after, as a job")


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
    row = rows_by_key(form)["heroic-gog"]
    assert row["state"] == "failed" and "/mnt/games/PC" in row["display"] and owner in row["display"], "no scan_dirs write, so no scan"
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
    until(lambda: opened() and not form.busy and form.stepId == "found" and form.count == 5)
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
