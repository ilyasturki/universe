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
    assert [s["id"] for s in form.steps] == ["found", "stores", "install", "data", "preferences", "done"]
    assert form.stepId == "found"
    rows = rows_by_key(form)
    assert form.rows[0]["key"] == "everything", "Add everything leads"
    assert {key: (row["type"], row["via"]) for key, row in rows.items()} == {
        "everything": ("action", "everything"),
        "lutris": ("action", "lutris"),
        "steam": ("action", "steam"),
        "heroic-gog": ("action", "gog"),
        "heroic-epic": ("action", "epic"),
        "roms": ("action", "roms"),
    }, "Heroic's Amazon games, with nothing to bring over, stay out"
    assert all((row["action"], row["verb"]) == ("Add", True) and row["detail"] for row in rows.values()), "one verb, and where the games go"
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
    assert [s["id"] for s in form.steps] == ["found", "install", "data", "preferences", "done"], "every store signed in: nothing to do there"


def test_an_offered_store_turns_on_and_adopts_its_launchers_games_once_signed_in(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    assert [s["id"] for s in form.steps] == ["found", "stores", "install", "data", "preferences", "done"], "Epic is off, its launcher is here"
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


def test_add_everything_runs_every_launcher_once(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    assert form.runImport(index_of(form, "everything")) is True
    assert "everything" not in rows_by_key(form), "nothing is left to add"

    def states():
        return {row["key"]: row["state"] for row in form.rows if "state" in row}

    until(lambda: len(states()) == 5 and set(states().values()) <= {"imported", "waiting"}, "each runs to its end")
    assert states()["heroic-epic"] == "waiting", "Epic, off and signed out, waits for its sign-in"
    until(lambda: empty_api.allGames.count == 3, "Lutris's two and the emulator folders' one, the importers one after the other")


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
    empty.core._data["sources"] = [s for s in empty.core._data["sources"] if s["id"] != "gog"]
    errors = record(empty.error)
    form = loaded(empty_api.screens.onboarding)
    assert errors == [], "no gog source: its folders are not asked for"
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


def test_the_install_folder_is_the_games_root_or_another_launchers(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    while form.stepId != "install":
        form.next()
    assert [(r["key"], r["type"], r.get("dir")) for r in form.rows] == [
        ("install_dir", "static", "/mnt/games/PC"),
        ("install_dir", "action", "/mnt/games/Heroic"),
        ("install_dir", "action", "/home/player/Games"),
        ("paths.games_root", "path", None),
    ], "the folder in use, then Heroic's and Lutris's, then any other"
    assert form.runImport(1) is True
    assert empty.core.settings()["paths"]["games_root"] == "/mnt/games/Heroic"
    assert [r["type"] for r in form.rows[:2]] == ["action", "static"], "the one picked is the one in use"
    assert form.setValue(index_of(form, "paths.games_root"), "/srv/games") is True
    assert empty.core.settings()["paths"]["games_root"] == "/srv/games"
    assert [r["dir"] for r in form.rows if r["type"] == "static"] == ["/srv/games"], "a folder of one's own joins the list, in use"


def test_the_data_step_sets_the_prefixes_and_save_backups_roots(empty_api, empty):
    form = loaded(empty_api.screens.onboarding)
    while form.stepId != "data":
        form.next()
    assert [(r["key"], r["type"]) for r in form.rows] == [("paths.prefixes_root", "path"), ("paths.saves_root", "path"), ("saves.auto_backup", "bool")]
    assert form.setValue(index_of(form, "paths.saves_root"), "/srv/saves") is True
    assert empty.core.settings()["paths"]["saves_root"] == "/srv/saves"
    form.toggle(index_of(form, "saves.auto_backup"))
    assert empty.core.settings()["saves"]["auto_backup"] is False


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
    assert [s["id"] for s in form.steps] == ["found", "stores", "done"], "no install folder nor preferences: nothing to write them to"
    rows = rows_by_key(form)
    assert rows["heroic-gog"]["type"] == "static" and "/mnt/games/PC" in rows["heroic-gog"]["detail"] and owner in rows["heroic-gog"]["detail"]
    assert rows["heroic-epic"]["type"] == "static" and "sources.enabled" in rows["heroic-epic"]["detail"], "Epic is off, and cannot be turned on"
    assert form.runImport(index_of(form, "heroic-gog")) is False, "no scan_dirs write, so no scan"
    assert "scan_dirs" not in empty.core._config.get("sources", {}).get("gog", {})
    form.next()
    assert form.stepId == "stores" and "enabled" not in [r["key"] for r in form.rows], "no switch the config cannot keep"
    form.next()
    assert form.stepId == "done" and form.rows[-1]["key"] == "read_only" and owner in form.rows[-1]["detail"]


LOOKS = ["reprise", "switch2", "ps5"]


class Look:
    """The setup as one look draws it: its window, whether the page is up, a key pressed."""

    def __init__(self, api, theme):
        from PySide6.QtCore import QObject

        self.theme = theme
        api.theme.set(theme)
        api.theme.takeLanding()
        self.engine, self.window = render(api, activate=True)
        self.root = self.window.property("contentItem").childItems()[0].property("item")
        self.home = until(lambda: self.root.property("activePage") if theme == "reprise" else self.root.findChild(QObject, "homePage"))

    def close(self):
        """The window closed and its engine dropped now, not whenever the collector gets to it."""
        self.window.close()
        del self.engine

    def opened(self):
        if self.theme == "reprise":
            return self.root.property("subOpen") is True and self.root.property("subSource") == "pages/OnboardingPage.qml"
        top = self.root.property("topPage")
        return top is not None and top.property("form") is not None and top.property("last") is False

    def focused(self, name):
        """Whether the page's `setupNav` (Back and Continue) or `setupRows` has the focus."""
        from PySide6.QtCore import QObject

        page = self.root if self.theme == "reprise" else self.root.property("topPage")
        # A closed page's items linger until deleted; only the live one can hold the focus.
        return page is not None and any(item.property("activeFocus") is True for item in page.findChildren(QObject, name))

    def nav_focused(self):
        return self.focused("setupNav")

    def rows_focused(self):
        return self.focused("setupRows")

    def press(self, key, times=1):
        from PySide6.QtTest import QTest

        for _ in range(times):
            QTest.keyClick(self.window, key)

    def pick_second(self):
        """The second answer of the look's question: down its list in Reprise, right along its buttons on the stack looks."""
        from PySide6.QtCore import Qt

        self.press(Qt.Key.Key_Down if self.theme == "reprise" else Qt.Key.Key_Right)
        self.press(Qt.Key.Key_Return)

    def reopen(self):
        """The setup again: the empty Home's Set up entry, Settings › About on the PS5 look."""
        from PySide6.QtCore import Q_ARG, QMetaObject, Qt

        if self.theme == "ps5":
            QMetaObject.invokeMethod(self.root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "about"}))
            until(lambda: (top := self.root.property("topPage")) is not None and top.property("sectionId") == "about" and top.property("activeFocus"))
            self.press(Qt.Key.Key_Down)
            self.press(Qt.Key.Key_Right)
        else:
            if self.theme == "reprise":
                self.press(Qt.Key.Key_Up)
            self.press(Qt.Key.Key_Right)
            until(lambda: self.home.property("onSetup") is True)
        self.press(Qt.Key.Key_Return)


@pytest.mark.parametrize("theme", LOOKS)
def test_the_setup_in_each_look(empty_api, empty, theme):
    from PySide6.QtCore import Qt

    signed_out(empty)
    look = Look(empty_api, theme)
    form = empty_api.screens.onboarding
    until(lambda: look.opened() and not form.loading and form.stepId == "found" and form.count == 6)
    look.press(Qt.Key.Key_I)
    until(lambda: form.stepId == "stores", "X moves on")
    look.press(Qt.Key.Key_Escape)
    until(lambda: form.stepId == "found" and look.opened(), "B goes back a step, the setup stays")
    look.press(Qt.Key.Key_Down, 7)
    until(look.nav_focused, "Down past the last row reaches the buttons")
    look.press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "stores", "A on Continue moves on")
    look.press(Qt.Key.Key_Down, 5)
    look.press(Qt.Key.Key_Left)
    look.press(Qt.Key.Key_Return)
    until(lambda: form.stepId == "found", "the Back button goes back")
    look.press(Qt.Key.Key_Escape)
    look.press(Qt.Key.Key_Return)
    until(lambda: look.opened() and form.stepId == "found" and not empty.onboarded(), "B on the first step asks first: Keep going stays")
    look.press(Qt.Key.Key_Escape)
    look.pick_second()
    until(lambda: empty.onboarded() is True and not look.opened(), "Skip skips it")
    look.reopen()
    until(lambda: look.opened() and form.stepId == "found", "it runs again")
    until(look.rows_focused)
    look.press(Qt.Key.Key_Down)
    look.press(Qt.Key.Key_Return)
    until(lambda: empty_api.allGames.count == 2, "A on the Lutris row imports behind the setup")
    assert form.added is True
    look.press(Qt.Key.Key_Escape)
    until(lambda: not look.opened(), "with games in, B closes without asking")
    home = look.home
    if theme == "reprise":
        until(
            lambda: home.property("tileSelected") is False and (game := home.property("currentGame")) is not None and game.property("id") is not None,
            "the rail that filled behind the dialog lands on a game",
        )
        look.press(Qt.Key.Key_Down)
        look.press(Qt.Key.Key_Right, 2)
        until(lambda: home.property("tileSelected") is True, "Down leaves the hero pills for the rail, Right past the last game reaches the Library tile")
        look.press(Qt.Key.Key_Left)
        until(lambda: home.property("tileSelected") is False and home.property("currentGame") is not None)
    elif theme == "switch2":
        until(lambda: home.property("onSetup") is False and home.property("index") <= home.property("allIndex"))
    look.close()


@pytest.mark.parametrize("theme", LOOKS)
def test_a_step_with_nothing_to_do_lands_on_its_button(empty_api, empty, theme):
    from PySide6.QtCore import Qt

    look = Look(empty_api, theme)
    form = empty_api.screens.onboarding
    until(lambda: look.opened() and not form.loading and form.stepId == "found")
    until(look.rows_focused, "the found step's rows come first")
    for step in range(1, len(form.steps)):
        look.press(Qt.Key.Key_I)
        until(lambda step=step: form.step == step, "X moves on")
    until(lambda: form.idle is True and look.nav_focused(), "the summary does nothing: Finish has the focus")
    look.press(Qt.Key.Key_Return)
    until(lambda: empty.onboarded() is True and not look.opened(), "A finishes")
    look.close()
