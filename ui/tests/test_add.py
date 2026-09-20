from conftest import rows_by_key, settle, wait_for
from universe_ui.screens.add import runner_candidates


def test_add_form_rows(api, fake):
    form = api.screens.add
    form.load()
    assert [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups] == [("", ["pick_file"]), ("Stores", ["store"]), ("Lutris", ["lutris"])]
    rows = rows_by_key(form)
    assert rows["pick_file"]["type"] == "action" and rows["pick_file"]["action"] == "Pick a file"
    assert rows["store"]["label"] == "GOG" and rows["store"]["source"] == "gog" and rows["store"]["action"] == "Open"
    assert rows["store"]["display"] == "Signed in as yasso" and rows["store"]["loggedIn"] is True and rows["store"]["available"] is True
    assert rows["lutris"]["display"] == "" and rows["lutris"]["action"] == "Import"


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
    assert form.runnerChoices[:2] == ["Proton", "Wine"] and form.runnerIndex == 0
    assert form.pendingTitle() == "hollow knight"
    form.pickRunner(1)
    assert form.runnerIds[form.runnerIndex] == "wine"
    messages = []
    form.message.connect(messages.append)
    ident = form.addGame("Hollow Knight")
    assert ident == "hollow-knight" and form.pendingFile == ""
    assert messages == ["Added Hollow Knight through Wine"]
    game = fake.game(ident)
    assert game["launch"] == {"runner": "wine", "exe": "/games/Hollow_Knight/hollow_knight [GOG].exe"}
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
    assert rows_by_key(form)["lutris"]["display"] == "2 games to import"
    messages, errors = [], []
    form.message.connect(messages.append)
    fake.error.connect(lambda kind, message: errors.append(kind))
    form.importLutris()
    assert wait_for(fake.libraryChanged) == ([],), "a full reload: the import may have updated games too"
    settle(form)
    assert messages == ["Imported 2 games from Lutris · 43 h of play"] and errors == []
    assert form.lutris is None and rows_by_key(form)["lutris"]["display"] == ""
    assert api.allGames.byId("celeste") is not None and api.allGames.byId("hades") is not None


def test_lutris_absent_is_shown_not_toasted(api, fake):
    fake.core._data.pop("lutris")
    form = api.screens.add
    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    form.load()
    form.previewLutris()
    settle(form)
    assert form.lutris is None and form.lutrisError.endswith("pga.db")
    assert rows_by_key(form)["lutris"]["display"] == "Not found"
    assert errors == []
    form.importLutris()
    assert not form.busy, "nothing to apply"
