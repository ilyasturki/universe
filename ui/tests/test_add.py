import threading

from looks import focus_names, held_notices, invoke
from PySide6.QtCore import SIGNAL, QObject, Qt
from uitest import record, settle, until

from universe_ui.errors import UniverseError
from universe_ui.screens.add import runner_candidates


def test_add_form_rows(api, fake):
    form = api.screens.add
    form.load()
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows))), "every row sits in one card"
    kinds = [{form.rows[i]["key"] for i in g["rows"]} for g in form.groups]
    assert sorted(kinds, key=min) == [{"bring_back"}, {"lutris"}, {"pick_file"}, {"store"}], "a card for each way in"
    stores = {r["source"]: (r["loggedIn"], r["available"]) for r in form.rows if r["key"] == "store"}
    assert stores == {"gog": (True, True), "epic": (False, False), "itch": (False, False), "steam": (False, False)}, "off: set it up first"


def test_bring_back_lists_hidden_and_removed_games_and_goes_with_the_last(api, fake):
    form = api.screens.add
    fake.core.remove("the-technomancer", False)
    form.load()
    assert {r["game"] for r in form.rows if r["key"] == "bring_back"} == {"cyberpunk-2077", "the-technomancer"}
    said = record(form.message)
    form.bringBack("the-technomancer")
    form.bringBack("cyberpunk-2077")
    assert len(said) == 2 and api.allGames.byId("the-technomancer") is not None and api.allGames.byId("cyberpunk-2077") is not None
    assert not [r for r in form.rows if r["key"] == "bring_back"]
    assert not [g for g in form.groups if not g["rows"]], "no card is left empty"


def test_runner_proposed_from_the_file(api, fake):
    runners = fake.runners()
    ids = lambda path: [r["id"] for r in runner_candidates(runners, path)]
    assert ids("/games/Celeste/Celeste.exe")[:2] == ["proton", "wine"], "Proton before Wine for a Windows program"
    assert ids("/roms/homebrew.elf")[:2] == ["dolphin", "rpcs3"], "the found runner ahead of the missing one"
    assert ids("/games/factorio/bin/x64/factorio")[0] == "linux"
    assert ids("/games/Game.AppImage")[0] == "linux"
    assert ids("/roms/pokemon.nds")[0] == "melonds"
    assert ids("/roms/game.nsp")[0] == "eden"
    assert len(ids("/x/unknown.zzz")) == len(runners)


def test_add_game_flow(api, fake):
    form = api.screens.add
    form.load()
    assert form.setFile("") is False and form.pendingFile == ""
    assert form.setFile("/games/Hollow_Knight/hollow_knight [GOG].exe") is True
    assert form.runnerIds[:2] == ["proton", "wine"] and form.runnerIndex == 0
    assert form.pendingTitle() == "hollow knight"
    form.pickRunner(1)
    assert form.runnerIds[form.runnerIndex] == "wine"
    said = record(form.message)
    ident = form.addGame("Hollow Knight")
    assert ident == "hollow-knight" and form.pendingFile == "" and len(said) == 1
    game = fake.game(ident)
    assert {k: v for k, v in game["launch"].items() if v not in ("", None, [], {})} == {"runner": "wine", "exe": "/games/Hollow_Knight/hollow_knight [GOG].exe"}
    assert form.addGame("again") == "", "nothing pending"
    form.setFile("/x/y.exe")
    form.cancel()
    assert form.pendingFile == "" and form.addGame("y") == ""


def test_lutris_preview_then_import(api, fake):
    form = api.screens.add
    form.load()
    form.previewLutris()
    settle(form)
    assert form.lutris["imported"] == ["celeste", "hades"] and form.lutrisError == ""
    said, errors = record(form.message), record(fake.error)
    changed = record(fake.libraryChanged)
    form.importLutris()
    assert until(lambda: changed)[0] == ([],), "a full reload: the import may have updated games too"
    settle(form)
    assert len(said) == 1 and errors == [] and form.lutris is None
    assert api.allGames.byId("celeste") is not None and api.allGames.byId("hades") is not None


def test_lutris_absent_is_shown_not_toasted(api, fake):
    fake.core._data.pop("lutris")
    form = api.screens.add
    errors = record(fake.error)
    form.load()
    form.previewLutris()
    settle(form)
    assert form.lutris is None and form.lutrisError.endswith("pga.db") and errors == []
    form.importLutris()
    assert not form.busy, "nothing to apply"


def test_a_lutris_import_reports_once_whether_its_page_stays_or_closes(api, fake, look, monkeypatch):
    form = api.screens.add
    gate, real, failing = threading.Event(), fake.core.import_lutris, []

    def held(apply):
        if apply:
            gate.wait(10)
            if failing:
                raise UniverseError("Io", "pga.db is locked")
        return real(apply)

    monkeypatch.setattr(fake.core, "import_lutris", held)
    posted, clear = held_notices(look.engine)

    def import_lutris(close):
        gate.clear()
        clear()
        page = look.open("pages/AddGamePage.qml", {"add": True})
        form.previewLutris()
        settle(form)
        invoke(page, "importLutris")
        dialog = look.dialog() if look.stacked else page.findChild(QObject, "dialog")
        until(lambda: dialog.property("open") is True, "the import asks first")
        look.press(Qt.Key.Key_Return)
        until(lambda: form.busy, "Import starts it")
        while close and "addGamePage" in focus_names(look.window):
            look.press(Qt.Key.Key_Escape)
        if not close:
            assert form.receivers(SIGNAL("message(QString)")) == 1, "Notices folds a repeated text, so one listener is the guard"
            assert form.receivers(SIGNAL("failed(QString)")) == 1, "and one for a failure"
        gate.set()
        until(lambda: posted() and not form.busy, "the result reaches the notices")
        return [(n["error"], n["text"]) for n in posted()]

    failing.append(True)
    [(error, text)] = import_lutris(close=True)
    assert error and "pga.db is locked" in text, "the failure outlives its page"
    assert [error for error, _ in import_lutris(close=False)] == [True], "one error with the page still open"
    failing.clear()
    assert [error for error, _ in import_lutris(close=True)] == [False], "so does the success"
    assert [error for error, _ in import_lutris(close=False)] == [False], "one notice with the page still open"
