from conftest import record, settle, until
from universe_ui.screens.add import runner_candidates


def test_add_form_rows(api, fake):
    form = api.screens.add
    form.load()
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows))), "every row sits in one card"
    kinds = [{form.rows[i]["key"] for i in g["rows"]} for g in form.groups]
    assert sorted(kinds, key=min) == [{"lutris"}, {"pick_file"}, {"store"}], "a card for each way in"
    stores = {r["source"]: (r["loggedIn"], r["available"]) for r in form.rows if r["key"] == "store"}
    assert stores == {"gog": (True, True), "epic": (False, False), "itch": (False, False), "steam": (False, False)}, "off: set it up first"


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
