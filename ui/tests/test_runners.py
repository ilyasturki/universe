from conftest import index_of, record, rows_by_key, settle, until
from universe_ui.screens.runners import suggested_title


# The Launch card and, behind Y, its advanced half.
def launch_keys(form):
    return [form.rows[i]["key"] for g in form.basicGroups + form.advancedGroups if g["title"] == "Launch" for i in g["rows"]]


def cards(form):
    return [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups]


def card_of(form, key):
    return [form.rows[i].get(key) for g in form.groups for i in g["rows"] if form.rows[i]["key"] == "component"]


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
    assert [rows[i]["display"] for i in ("proton", "eden", "dolphin", "linux")] == ["7 games", "1 game", "1 game", ""]
    assert rows["proton"]["action"] == "Open" and rows["proton"]["icon"] == "assets/runners/proton.svg"
    assert rows["proton"]["component"] == "ge-proton" and rows["proton"]["detail"].startswith("GE-Proton11-7"), "the Proton build in use"
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


def test_runner_form_cards(api, fake):
    with_builds(api)
    form = api.screens.runner
    form.load("dolphin")
    assert form.info["name"] == "Dolphin" and form.info["warning"] == "" and form.info["icon"] == "assets/runners/dolphin.svg"
    assert form.info["meta"] == "Nintendo GameCube, Nintendo Wii · /run/current-system/sw/bin/dolphin-emu"
    assert cards(form) == [
        ("Runner", ["exe", "args", "gamescope"]),
        ("Builds", ["component"]),
        ("Options", ["batch", "user_directory"]),
        ("Games", ["game", "add_file"]),
    ], "every card named, for the sidebar"
    assert card_of(form, "component") == ["dolphin"] and "build" not in rows_by_key(form), "the Builds card picks the build"
    rows = rows_by_key(form)
    game = rows["game"]
    assert game["type"] == "action" and game["action"] == "Options" and game["gameId"] == "lego-batman" and game["label"] == "LEGO Batman: The Videogame"
    assert game["image"].startswith("file://") and game["display"] == "1.1 h" and game["installed"] is False
    assert next(g for g in form.groups if g["title"] == "Games")["meta"] == "1 game"
    assert rows["exe"]["type"] == "path" and rows["exe"]["inherited"] is True and rows["exe"]["origin"] == "" and not form.resettable(rows["exe"])
    assert rows["exe"]["value"].endswith("dolphin-emu") and rows["exe"]["display"] == rows["exe"]["value"], "the found program is the value shown"
    assert rows["exe"]["detail"] == "Found on PATH"
    assert rows["gamescope"]["type"] == "bool" and rows["gamescope"]["inherited"] is True and rows["gamescope"]["origin"] == "global"
    assert rows["batch"]["type"] == "bool" and rows["batch"]["value"] is True
    assert rows["add_file"]["type"] == "action" and rows["add_file"]["runner"] == "dolphin"
    form.toggle(index_of(form, "gamescope"))
    rows = rows_by_key(form)
    assert rows["gamescope"]["value"] is False and rows["gamescope"]["origin"] == "runner" and form.resettable(rows["gamescope"]), (
        "toggling the inherited switch sets it on the runner"
    )
    assert form.reset(index_of(form, "gamescope")) is True and rows_by_key(form)["gamescope"]["origin"] == "global"
    assert rows_by_key(form)["gamescope"]["value"] is True, "X drops the runner's own value: the global's again"
    form.load("rpcs3")
    assert form.info["warning"] == "not found"
    form.load("melonds")
    rows = rows_by_key(form)
    assert "(config)" in form.info["meta"] and rows["exe"]["inherited"] is False and rows["exe"]["origin"] == "runner"
    assert form.reset(index_of(form, "exe")) is True and rows_by_key(form)["exe"]["origin"] == "" and form.info["warning"] == "not found", (
        "X drops the configured program; nothing was detected to fall back on"
    )
    form.load("linux")
    assert cards(form) == [("Runner", ["gamescope"]), ("Games", ["add_file"])], (
        "the program is the game itself, nothing to install; no games yet, the row that adds one"
    )
    assert form.groups[1]["meta"] == ""
    form.load("nope")
    assert form.rows == [] and form.info == {}


def test_the_builds_card_installs_and_switches_builds(api, fake):
    with_builds(api)
    form = api.screens.runner
    components = api.screens.components
    form.load("proton")
    assert cards(form)[:2] == [("Runner", ["exe", "component", "args", "gamescope"]), ("Builds", ["component", "component"])]
    assert card_of(form, "component") == ["umu-run", "ge-proton", "proton-cachyos"], "umu-run starts Proton's builds: beside its program"
    assert "launch.proton" not in rows_by_key(form), "the Builds card picks the Proton build"
    finished = record(fake.jobFinished)
    assert components.act("proton-cachyos", "install") is True
    until(lambda: form.rows[next(i for i, r in enumerate(form.rows) if r.get("component") == "proton-cachyos")]["action"] == "Cancel")
    until(lambda: finished)
    settle(components)
    cachy = until(lambda: next(r for r in form.rows if r.get("component") == "proton-cachyos" and r["accent"] is False and r["display"] != "Not installed"))
    assert cachy["key"] == "component"
    form.load("xemu")
    assert card_of(form, "component") == ["xemu"]
    assert components.act("xemu", "update") is True
    until(lambda: len(finished) == 2)
    settle(components)
    until(lambda: "0.8.136" in next(r for r in form.rows if r.get("component") == "xemu")["display"], "the build in use, updated")


def test_runner_form_carries_its_launch_keys(api, fake):
    form = api.screens.runner
    form.load("proton")
    assert cards(form) == [
        ("Runner", ["exe", "args", "gamescope"]),
        ("Proton", ["launch.wayland", "launch.hdr"]),
        ("Games", ["game"] * 7 + ["add_file"]),
    ], "config.toml's [launch] keys tied to Proton, the global values, its games; no Advanced row"
    assert not form.showAdvanced and form.hasAdvanced and "advanced" not in [r["key"] for r in form.rows]
    form.showAdvanced = True
    assert cards(form)[1] == (
        "Proton",
        [
            "launch.wayland",
            "launch.hdr",
            "launch.esync",
            "launch.fsync",
            "launch.ntsync",
            "launch.dlss_upgrade",
            "launch.fsr4_upgrade",
            "launch.xess_upgrade",
            "launch.optiscaler",
            "launch.debug_log",
        ],
    ), "the switches fold into the Proton card"
    assert form.groups[1]["dividers"] == [{"at": 2, "label": "Advanced · Sync"}, {"at": 5, "label": "Upscaling"}, {"at": 9, "label": "Logs"}]
    assert [g["title"] for g in form.groups] == ["Runner", "Proton", "Games"], "the sidebar does not move with Advanced"
    assert all(form.rows[i]["advanced"] for g in form.advancedGroups for i in g["rows"]) and all(g["advanced"] for g in form.advancedGroups)
    assert {g["title"]: g["meta"] for g in form.advancedGroups}["Upscaling"] == "AMD Radeon RX 7900 GRE · RDNA 3"
    rows = rows_by_key(form)
    assert rows["launch.esync"]["value"] is True and rows["launch.esync"]["inherited"] is False
    assert rows["launch.dlss_upgrade"]["detail"].endswith("an anti-cheat. Not for your GPU.")
    assert rows["launch.fsr4_upgrade"]["detail"].endswith("Works on your GPU.") and rows["launch.optiscaler"]["detail"].endswith("Works on your GPU.")
    assert (rows["launch.fsr4_upgrade"]["value"], rows["launch.fsr4_upgrade"]["display"]) == ("off", "off"), "a DLL swap is opt-in"
    assert rows["launch.fsr4_upgrade"]["choices"] == ["Default · off", "auto", "on", "off"]
    assert form.setValue(index_of(form, "launch.fsr4_upgrade"), "auto") is True
    assert fake.config()["launch"]["fsr4_upgrade"] == "auto" and rows_by_key(form)["launch.fsr4_upgrade"]["display"] == "auto · Off", (
        "auto says what it comes to on this GPU"
    )
    assert form.setValue(index_of(form, "launch.fsr4_upgrade"), "on") is True
    assert fake.config()["launch"]["fsr4_upgrade"] == "on" and rows_by_key(form)["launch.fsr4_upgrade"]["value"] == "on"
    form.load("wine")
    assert not form.showAdvanced, "another runner opens collapsed"
    assert form.reveal("launch.fsync", "") == index_of(form, "launch.fsync") and form.showAdvanced, "revealing an advanced row opens Advanced"
    assert cards(form) == [
        ("Runner", ["exe", "args", "gamescope", "launch.esync", "launch.fsync", "launch.debug_log"]),
        ("Games", ["add_file"]),
    ], "no NTSync, no Proton card on plain Wine: the switches fold into the runner's own card"
    assert form.groups[0]["dividers"] == [{"at": 3, "label": "Advanced · Sync"}, {"at": 5, "label": "Logs"}]
    form.load("wine")
    assert not form.showAdvanced, "reopening the same runner starts collapsed too"
    form.load("dolphin")
    assert "launch.esync" not in rows_by_key(form)
    fake.core._data["gpu"] = None
    form.load("proton")
    assert {g["title"]: g["meta"] for g in form.advancedGroups}["Upscaling"] == "" and rows_by_key(form)["launch.dlss_upgrade"]["detail"].endswith(
        "an anti-cheat."
    )


def test_runner_form_writes_through(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    batch = index_of(form, "batch")
    form.toggle(batch)
    assert form.rows[batch]["value"] is False
    assert fake.runners()[3]["options"][0]["value"] is False
    form.load("rpcs3")
    exe = index_of(form, "exe")
    assert form.setValue(exe, "/opt/rpcs3/rpcs3") is True
    assert form.info["warning"] == "" and "/opt/rpcs3/rpcs3 (config)" in form.info["meta"]


def test_add_game_flow(api, fake):
    api.screens.runners.load()
    form = api.screens.runner
    form.load("dolphin")
    messages = []
    form.message.connect(messages.append)
    add = index_of(form, "add_file")
    assert form.setValue(add, "") is False
    assert form.setValue(add, "/mnt/games/gamecube/Mario Kart - Double Dash!! (USA) [v1.1].iso") is True
    assert form.pendingTitle() == "Mario Kart - Double Dash!!"
    ident = form.addGame("Mario Kart: Double Dash")
    assert ident == "mario-kart-double-dash"
    assert messages == ["Added Mario Kart: Double Dash through Dolphin"]
    game = fake.game(ident)
    assert game["platform"] == "Nintendo GameCube"
    assert game["effective"]["runner"] == "dolphin" and game["effective"]["runner_name"] == "Dolphin"
    assert api.allGames.byId(ident) is not None, "the library picked the new game up"
    games = next(g for g in form.groups if g["title"] == "Games")
    assert [form.rows[i].get("gameId") for i in games["rows"]] == ["lego-batman", ident, None], "the page followed the library change"
    assert form.addGame("again") == "", "nothing pending twice"
    add = index_of(form, "add_file")
    assert form.setValue(add, "/x/a.iso") and form.addGame("Mario Kart: Double Dash") == ""
    assert api.screens.runners.rows[1]["runner"] == "dolphin", "two games now, ahead of Eden's one, without a reload"


def test_runner_form_removes_a_game(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    messages = []
    form.message.connect(messages.append)
    changed = record(fake.libraryChanged)
    form.remove("lego-batman")
    until(lambda: messages)
    assert messages == ["Removed LEGO Batman: The Videogame from the library"]
    until(lambda: changed)
    assert not any(r.get("gameId") == "lego-batman" for r in form.rows), "the Games group followed the library change"
    form.uninstall("")
    assert messages[1:] == [], "no game, no call"


def test_suggested_titles():
    assert suggested_title("/g/F-Zero GX.iso") == "F-Zero GX"
    assert suggested_title("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci") == "SUPER MARIO ODYSSEY Eur SuperXCi - CLC"
    assert suggested_title("") == ""


def test_game_settings_launch_group_by_runner(api, fake):
    form = api.screens.gameSettings
    form.load("mini-metro")
    assert launch_keys(form) == [
        "launch.runner",
        "launch.exe",
        "launch.runner_exe",
        "launch.options.fullscreen",
        "launch.wrapper",
        "launch.args",
        "launch.working_dir",
        "launch.pre_command",
        "launch.post_command",
    ]
    rows = rows_by_key(form)
    assert rows["launch.runner"]["value"] == "Eden" and rows["launch.runner"]["valueIcon"] == "assets/runners/eden.svg"
    assert rows["launch.runner"]["choices"][:4] == ["Proton", "Wine", "Linux", "Dolphin"]
    assert rows["launch.runner"]["choiceValues"][:4] == ["proton", "wine", "linux", "dolphin"]
    assert rows["launch.exe"]["label"] == "File" and rows["launch.exe"]["value"].endswith("Mini Metro.nsp")
    assert rows["launch.runner_exe"]["inherited"] is True and rows["launch.runner_exe"]["value"].endswith("/eden")
    assert rows["launch.options.fullscreen"]["value"] is False and rows["launch.options.fullscreen"]["inherited"] is False
    assert "launch.proton" not in rows and "platform" not in rows, "one platform: no Platform row"

    form.load("lego-batman")
    rows = rows_by_key(form)
    assert rows["platform"]["type"] == "enum" and rows["platform"]["value"] == "Nintendo Wii"
    assert rows["platform"]["choices"] == ["Nintendo GameCube", "Nintendo Wii"]

    form.load("the-technomancer")
    assert launch_keys(form) == [
        "launch.runner",
        "launch.exe",
        "launch.wrapper",
        "launch.args",
        "launch.working_dir",
        "launch.pre_command",
        "launch.post_command",
    ]
    form.showAdvanced = True
    assert [c for c in cards(form) if c[0] in ("Proton", "Sync", "Upscaling")] == [
        (
            "Proton",
            [
                "launch.proton",
                "launch.wayland",
                "launch.hdr",
                "launch.prefix",
                "launch.umu_id",
                "launch.store",
                "launch.dll_overrides",
                "launch.esync",
                "launch.fsync",
                "launch.ntsync",
                "launch.dlss_upgrade",
                "launch.fsr4_upgrade",
                "launch.xess_upgrade",
                "launch.optiscaler",
                "launch.debug_log",
            ],
        ),
    ], "the Proton card's advanced half folds into it behind Y, the sync and upscaling switches after it"
    rows = rows_by_key(form)
    assert rows["launch.runner"]["value"] == "Proton" and rows["launch.exe"]["label"] == "Program"
    assert rows["launch.wayland"]["value"] is True and rows["launch.wayland"]["inherited"] is True
    assert rows["launch.hdr"]["value"] is False and rows["launch.hdr"]["inherited"] is True

    index = index_of(form, "launch.runner")
    assert form.setValue(index, "Dolphin") is True
    assert fake.game("the-technomancer")["launch"]["runner"] == "dolphin"
    assert "platform" in launch_keys(form)
