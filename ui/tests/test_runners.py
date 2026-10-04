from conftest import index_of, record, rows_by_key, settle, until
from universe_ui.screens.runners import suggested_title


# The Launch card and, behind Y, its advanced half.
def launch_keys(form):
    return [form.rows[i]["key"] for g in form.basicGroups + form.advancedGroups if g["title"] == "Launch" for i in g["rows"]]


def cards(form):
    return {i: g["title"] for g in form.groups for i in g["rows"]}


def card_of(form, key):
    return cards(form)[index_of(form, key)]


def ids(form, group):
    return [form.rows[i].get("runner") or form.rows[i]["component"] for i in group["rows"]]


def with_builds(api):
    api.screens.components.load()
    settle(api.screens.components)


def test_runners_list_each_runner_with_its_builds_and_the_tools_last(api, fake):
    fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    form = api.screens.runners
    form.load()
    assert [ids(form, g) for g in form.groups] == [["proton", "eden", "dolphin", "linux", "melonds", "wine", "xemu"], ["rpcs3", "mame"]], (
        "before the catalogue: the runners found by the games on them, then their hours, then the name; the others"
    )
    assert form.groups[1]["off"] is True
    settle(api.screens.components)
    found, offered, none, tools = until(lambda: len(form.groups) == 4 and form.groups)
    assert ids(form, found) == ["proton", "eden", "dolphin", "linux", "melonds", "wine", "xemu"]
    assert ids(form, offered) == ["rpcs3"] and offered["off"] is False, "Universe can install it, and a game waits on it"
    assert ids(form, none) == ["mame"] and none["off"] is True, "no build to download"
    assert ids(form, tools) == ["gpu-screen-recorder", "gamescope"], "what to install first; umu-run sits on Proton's page"
    rows = {r.get("runner") or r["component"]: r for r in form.rows}
    assert rows["proton"]["component"] == "ge-proton", "the Proton build in use"
    assert rows["xemu"]["component"] == "xemu" and rows["xemu"]["accent"] is True and rows["xemu"]["size"], "an update waits"
    assert rows["rpcs3"]["accent"] is True and rows["rpcs3"]["size"]
    assert rows["linux"]["component"] == "" and rows["linux"]["accent"] is False, "nothing to install"
    assert rows["gamescope"]["key"] == "component" and "runner" not in rows["gamescope"], "a tool has no page"
    assert form.indexOf("dolphin") == 2 and form.indexOf("gamescope") == tools["rows"][1] and form.indexOf("nope") == -1


def test_an_install_moves_its_runner_up_and_the_row_shows_its_progress(api, fake):
    fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")
    form = api.screens.runners
    form.load()
    components = api.screens.components
    settle(components)
    until(lambda: len(form.groups) == 4)
    progress = []
    form.rowsChanged.connect(lambda: progress.append(form.rows[form.indexOf("rpcs3")]["progress"]))
    finished = record(fake.jobFinished)
    assert components.act("rpcs3", "install") is True
    until(lambda: finished)
    settle(components)
    until(lambda: form.indexOf("rpcs3") in form.groups[0]["rows"], "found once installed")
    assert [p for p in progress if p] == [0.25, 0.5, 0.75, 1.0]
    assert form.rows[form.indexOf("rpcs3")]["accent"] is False


def test_a_runners_page_holds_its_program_its_builds_and_its_games_and_writes_to_the_runner(api, fake):
    with_builds(api)
    form = api.screens.runner
    form.load("dolphin")
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows))), "every row sits in one card"
    assert all(g["title"] for g in form.groups), "every card named, for the sidebar"
    rows = rows_by_key(form)
    assert [r["component"] for r in form.rows if r["key"] == "component"] == ["dolphin"] and "build" not in rows, "the Builds card picks the build"
    assert card_of(form, "game") == card_of(form, "add_file") and rows["add_file"]["runner"] == "dolphin"
    game = rows["game"]
    assert game["gameId"] == "lego-batman" and game["image"].startswith("file://") and game["installed"] is False
    assert rows["exe"]["inherited"] is True and rows["exe"]["origin"] == "found" and not form.resettable(rows["exe"])
    assert rows["exe"]["value"].endswith("dolphin-emu") and rows["exe"]["display"] == rows["exe"]["value"], "the found program is the value shown"
    assert rows["gamescope"]["inherited"] is True and rows["gamescope"]["origin"] == "default"
    form.toggle(index_of(form, "gamescope"))
    rows = rows_by_key(form)
    assert rows["gamescope"]["value"] is False and rows["gamescope"]["origin"] == "runner" and form.resettable(rows["gamescope"]), (
        "toggling the inherited switch sets it on the runner"
    )
    assert form.reset(index_of(form, "gamescope")) is True and rows_by_key(form)["gamescope"]["origin"] == "default"
    assert rows_by_key(form)["gamescope"]["value"] is True, "X drops the runner's own value: the global's again"
    form.toggle(index_of(form, "batch"))
    assert rows_by_key(form)["batch"]["value"] is False
    assert next(r for r in fake.runners() if r["id"] == "dolphin")["options"][0]["value"] is False
    form.load("rpcs3")
    assert form.info["warning"], "no program found"
    assert form.setValue(index_of(form, "exe"), "/opt/rpcs3/rpcs3") is True and form.info["warning"] == ""
    form.load("melonds")
    rows = rows_by_key(form)
    assert rows["exe"]["inherited"] is False and rows["exe"]["origin"] == "runner" and not form.info["warning"]
    assert form.reset(index_of(form, "exe")) is True and rows_by_key(form)["exe"]["origin"] == "" and form.info["warning"], (
        "X drops the configured program; nothing was detected to fall back on"
    )
    form.load("linux")
    assert not {"exe", "component", "game"} & set(rows_by_key(form)) and "add_file" in rows_by_key(form), (
        "the program is the game itself, nothing to install; no games yet, the row that adds one"
    )
    form.load("nope")
    assert form.rows == [] and form.info == {}


def test_the_builds_card_installs_and_switches_builds(api, fake):
    with_builds(api)
    form = api.screens.runner
    components = api.screens.components
    form.load("proton")
    program, builds = card_of(form, "exe"), card_of(form, "launch.proton")
    assert {r["component"]: cards(form)[i] for i, r in enumerate(form.rows) if r["key"] == "component"} == {
        "umu-run": program,
        "ge-proton": builds,
        "proton-cachyos": builds,
    } and program != builds, "umu-run starts Proton's builds: beside its program"
    default = rows_by_key(form)["launch.proton"]
    assert default["value"] == "proton-ge"
    assert default["choices"] == ["Proton 9.0", "proton-cachyos", "proton-em", "proton-ge", "proton-tkg"], (
        "every Proton found is a default: Steam's own and config.toml's beside the families"
    )
    assert form.setValue(index_of(form, "launch.proton"), "Proton 9.0") is True and fake.config()["launch"]["proton"] == "Proton 9.0"
    finished = record(fake.jobFinished)
    assert components.act("proton-cachyos", "install") is True
    until(lambda: finished)
    settle(components)
    cachy = next(c for c in components.listing()["components"] if c["id"] == "proton-cachyos")
    row = next(r for r in form.rows if r.get("component") == "proton-cachyos")
    assert cachy["in_use"] and row["display"] == components.row(cachy)["display"], "the card shows the build installed"


def test_runner_form_carries_its_launch_keys(api, fake):
    form = api.screens.runner
    form.load("proton")
    titles = [g["title"] for g in form.groups]
    proton = card_of(form, "launch.wayland")
    assert card_of(form, "launch.hdr") == proton and proton != card_of(form, "exe"), "config.toml's [launch] keys tied to Proton, a card of their own"
    assert not form.showAdvanced and form.hasAdvanced and "advanced" not in rows_by_key(form)
    form.showAdvanced = True
    assert [g["title"] for g in form.groups] == titles, "the sidebar does not move with Advanced"
    assert {cards(form)[i] for g in form.advancedGroups for i in g["rows"]} == {proton}, "the switches fold into the Proton card"
    rows = rows_by_key(form)
    assert rows["launch.esync"]["value"] is True and rows["launch.esync"]["inherited"] is False
    fsr4 = rows["launch.fsr4_upgrade"]
    assert fsr4["value"] == fsr4["choices"][0] and fsr4["choiceValues"] == ["", "auto", "on", "off"], (
        "a DLL swap is opt-in: config.toml sets none, the picker opens on the default"
    )
    assert form.setValue(index_of(form, "launch.fsr4_upgrade"), "on") is True
    assert fake.config()["launch"]["fsr4_upgrade"] == "on" and rows_by_key(form)["launch.fsr4_upgrade"]["value"] == "on"
    form.load("wine")
    assert not form.showAdvanced, "another runner opens collapsed"
    assert form.reveal("launch.fsync", "") == index_of(form, "launch.fsync") and form.showAdvanced, "revealing an advanced row opens Advanced"
    assert {card_of(form, k) for k in ("launch.esync", "launch.fsync", "launch.debug_log")} == {card_of(form, "exe")}, (
        "no Proton card on plain Wine: the switches fold into the runner's own card"
    )
    assert "launch.ntsync" not in rows_by_key(form), "no NTSync on plain Wine"
    form.load("wine")
    assert not form.showAdvanced, "reopening the same runner starts collapsed too"
    form.load("dolphin")
    assert "launch.esync" not in rows_by_key(form)


def test_add_game_flow(api, fake):
    api.screens.runners.load()
    form = api.screens.runner
    form.load("dolphin")
    said = record(form.message)
    add = index_of(form, "add_file")
    assert form.setValue(add, "") is False
    assert form.setValue(add, "/mnt/games/gamecube/Mario Kart - Double Dash!! (USA) [v1.1].iso") is True
    assert form.pendingTitle() == "Mario Kart - Double Dash!!"
    ident = form.addGame("Mario Kart: Double Dash")
    assert ident == "mario-kart-double-dash" and len(said) == 1
    game = fake.game(ident)
    assert game["platform"] == "Nintendo GameCube"
    assert game["effective"]["runner"] == "dolphin"
    assert api.allGames.byId(ident) is not None, "the library picked the new game up"
    games = next(g for g in form.groups if g["title"] == card_of(form, "add_file"))
    assert [form.rows[i].get("gameId") for i in games["rows"]] == ["lego-batman", ident, None], "the page followed the library change"
    assert form.addGame("again") == "", "nothing pending twice"
    add = index_of(form, "add_file")
    assert form.setValue(add, "/x/a.iso") and form.addGame("Mario Kart: Double Dash") == ""
    assert api.screens.runners.rows[1]["runner"] == "dolphin", "two games now, ahead of Eden's one, without a reload"


def test_runner_form_removes_a_game(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    said = record(form.message)
    changed = record(fake.libraryChanged)
    form.remove("lego-batman")
    until(lambda: said and changed)
    assert not any(r.get("gameId") == "lego-batman" for r in form.rows), "the Games group followed the library change"
    form.uninstall("")
    assert len(said) == 1, "no game, no call"


def test_suggested_titles():
    assert suggested_title("/g/F-Zero GX.iso") == "F-Zero GX"
    assert suggested_title("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci") == "SUPER MARIO ODYSSEY Eur SuperXCi - CLC"
    assert suggested_title("") == ""


def test_a_games_launch_card_follows_its_runner(api, fake):
    form = api.screens.gameSettings
    form.load("mini-metro")
    rows = rows_by_key(form)
    runner = rows["launch.runner"]
    assert runner["choiceValues"][runner["choices"].index(runner["value"])] == "eden"
    assert {"launch.runner_exe", "launch.options.fullscreen"} <= set(launch_keys(form)), "an emulator's program and options sit in the Launch card"
    assert rows["launch.runner_exe"]["inherited"] is True and rows["launch.runner_exe"]["value"].endswith("/eden")
    assert rows["launch.options.fullscreen"]["value"] is False and rows["launch.options.fullscreen"]["inherited"] is False
    assert "launch.proton" not in rows and "platform" not in rows, "one platform: no Platform row"
    form.load("lego-batman")
    platform = rows_by_key(form)["platform"]
    assert (platform["type"], platform["value"], platform["choices"]) == ("enum", "Nintendo Wii", ["Nintendo GameCube", "Nintendo Wii"])
    form.load("the-technomancer")
    assert not {"launch.runner_exe", "platform"} & set(launch_keys(form))
    assert form.setValue(index_of(form, "launch.runner"), "Dolphin") is True
    assert fake.game("the-technomancer")["launch"]["runner"] == "dolphin"
    assert "platform" in launch_keys(form)
