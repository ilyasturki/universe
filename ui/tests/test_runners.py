from universe_ui.screens.runners import suggested_title


def rows_of(form):
    return {r["key"]: r for r in form.rows}


def launch_keys(form):
    launch = next(g for g in form.groups if g["title"] == "Launch")
    return [form.rows[i]["key"] for i in launch["rows"]]


def test_runners_list_by_usage(api, fake):
    form = api.screens.runners
    form.load()
    assert [r["runner"] for r in form.rows] == ["proton", "eden", "dolphin", "linux", "melonds", "wine", "rpcs3"], \
        "games linked, then hours, then the name; the ones not found last"
    assert [r["display"] for r in form.rows][:4] == ["7 games", "1 game", "1 game", ""]
    assert form.rows[0]["type"] == "action" and form.rows[0]["action"] == "Open" and form.rows[0]["label"] == "Proton"
    assert form.rows[2]["icon"] == "assets/runners/dolphin.svg"
    assert [g["title"] for g in form.groups] == ["", "Not found"]
    assert form.groups[0]["rows"] == [0, 1, 2, 3, 4, 5] and form.groups[1]["rows"] == [6] and form.groups[1]["off"] is True
    assert form.indexOf("dolphin") == 2 and form.indexOf("nope") == -1


def test_runner_form_cards(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    assert form.info["name"] == "Dolphin" and form.info["warning"] == "" and form.info["icon"] == "assets/runners/dolphin.svg"
    assert form.info["meta"] == "Nintendo GameCube, Nintendo Wii · /run/current-system/sw/bin/dolphin-emu"
    assert [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups] == \
        [("", ["exe", "args"]), ("", ["gamescope"]), ("Options", ["batch", "user_directory", "inputplumber"]), ("", ["add_file"])]
    rows = rows_of(form)
    assert rows["exe"]["type"] == "path" and rows["exe"]["inherited"] is True
    assert rows["exe"]["value"].endswith("dolphin-emu") and rows["exe"]["display"] == rows["exe"]["value"], "the found program is the value shown"
    assert rows["exe"]["detail"] == "Found on PATH"
    assert rows["gamescope"]["type"] == "bool" and rows["gamescope"]["inherited"] is True
    assert rows["batch"]["type"] == "bool" and rows["batch"]["value"] is True
    assert rows["add_file"]["type"] == "action" and rows["add_file"]["runner"] == "dolphin"
    form.load("rpcs3")
    assert form.info["warning"] == "not found"
    form.load("melonds")
    assert "(config)" in form.info["meta"] and rows_of(form)["exe"]["inherited"] is False
    form.load("linux")
    assert [form.rows[i]["key"] for g in form.groups for i in g["rows"]] == ["gamescope", "add_file"], "the program is the game itself"
    form.load("nope")
    assert form.rows == [] and form.info == {}


def test_runner_form_writes_through(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    batch = next(i for i, r in enumerate(form.rows) if r["key"] == "batch")
    form.toggle(batch)
    assert form.rows[batch]["value"] is False
    assert fake.runners()[3]["options"][0]["value"] is False
    form.load("rpcs3")
    exe = next(i for i, r in enumerate(form.rows) if r["key"] == "exe")
    assert form.setValue(exe, "/opt/rpcs3/rpcs3") is True
    assert form.info["warning"] == "" and "/opt/rpcs3/rpcs3 (config)" in form.info["meta"]


def test_add_game_flow(api, fake):
    form = api.screens.runner
    form.load("dolphin")
    messages = []
    form.message.connect(messages.append)
    add = next(i for i, r in enumerate(form.rows) if r["key"] == "add_file")
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
    assert form.addGame("again") == "", "nothing pending twice"
    assert form.setValue(add, "/x/a.iso") and form.addGame("Mario Kart: Double Dash") == ""
    api.screens.runners.load()
    assert api.screens.runners.rows[1]["runner"] == "dolphin", "two games now, ahead of Eden's one"


def test_suggested_titles():
    assert suggested_title("/g/F-Zero GX.iso") == "F-Zero GX"
    assert suggested_title("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci") == "SUPER MARIO ODYSSEY Eur SuperXCi - CLC"
    assert suggested_title("") == ""


def test_game_settings_launch_group_by_runner(api, fake):
    form = api.screens.gameSettings
    form.load("mini-metro")
    assert launch_keys(form) == ["launch.runner", "launch.exe", "launch.runner_exe", "launch.options.fullscreen", "launch.options.inputplumber",
                                 "launch.mangohud", "launch.wrapper", "launch.args", "launch.working_dir"]
    rows = rows_of(form)
    assert rows["launch.runner"]["value"] == "Eden" and rows["launch.runner"]["icon"] == "assets/runners/eden.svg"
    assert rows["launch.runner"]["choices"][:4] == ["Proton", "Wine", "Linux", "Dolphin"]
    assert rows["launch.runner"]["choiceValues"][:4] == ["proton", "wine", "linux", "dolphin"]
    assert rows["launch.exe"]["label"] == "File" and rows["launch.exe"]["value"].endswith("Mini Metro.nsp")
    assert rows["launch.runner_exe"]["inherited"] is True and rows["launch.runner_exe"]["value"].endswith("/eden")
    assert rows["launch.options.fullscreen"]["value"] is False and rows["launch.options.fullscreen"]["inherited"] is False
    assert rows["launch.options.inputplumber"]["value"] is True and rows["launch.options.inputplumber"]["inherited"] is True
    assert "launch.proton" not in rows and "platform" not in rows, "one platform: no Platform row"

    form.load("lego-batman")
    rows = rows_of(form)
    assert rows["platform"]["type"] == "enum" and rows["platform"]["value"] == "Nintendo Wii"
    assert rows["platform"]["choices"] == ["Nintendo GameCube", "Nintendo Wii"]

    form.load("the-technomancer")
    assert launch_keys(form) == ["launch.runner", "launch.exe", "launch.proton", "launch.esync", "launch.fsync", "launch.ntsync", "launch.wayland", "launch.hdr",
                                 "launch.dlss_upgrade", "launch.fsr4_upgrade", "launch.xess_upgrade", "launch.optiscaler", "launch.prefix",
                                 "launch.mangohud", "launch.wrapper", "launch.args", "launch.working_dir"]
    rows = rows_of(form)
    assert rows["launch.runner"]["value"] == "Proton" and rows["launch.exe"]["label"] == "Program"
    assert rows["launch.wayland"]["value"] is True and rows["launch.wayland"]["inherited"] is True
    assert rows["launch.hdr"]["value"] is False and rows["launch.hdr"]["inherited"] is True

    index = next(i for i, r in enumerate(form.rows) if r["key"] == "launch.runner")
    assert form.setValue(index, "Dolphin") is True
    assert fake.game("the-technomancer")["launch"]["runner"] == "dolphin"
    assert "platform" in launch_keys(form)
