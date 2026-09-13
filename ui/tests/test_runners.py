from universe_ui.screens.runners import suggested_title


def rows_of(form, runner):
    return {r["key"]: r for r in form.rows if r["module"] == runner}


def launch_keys(form):
    launch = next(g for g in form.groups if g["title"] == "Launch")
    return [form.rows[i]["key"] for i in launch["rows"]]


def test_runners_form_cards(api, fake):
    form = api.screens.runners
    form.load()
    groups = {g["title"]: g for g in form.groups}
    assert [g["title"] for g in form.groups][:4] == ["Proton", "Wine", "Linux", "Dolphin"]
    dolphin = groups["Dolphin"]
    assert dolphin["runner"] == "dolphin" and dolphin["warning"] == "" and dolphin["off"] is False
    assert dolphin["meta"] == "Nintendo GameCube, Nintendo Wii · /run/current-system/sw/bin/dolphin-emu"
    assert dolphin["icon"] == "assets/runners/dolphin.svg"
    assert [form.rows[i]["key"] for i in dolphin["rows"]] == ["exe", "args", "batch", "user_directory", "inputplumber", "add_file"]
    rows = rows_of(form, "dolphin")
    assert rows["exe"]["type"] == "path" and rows["exe"]["inherited"] is True and rows["exe"]["detail"].endswith("dolphin-emu")
    assert rows["batch"]["type"] == "bool" and rows["batch"]["value"] is True
    assert rows["add_file"]["type"] == "action" and rows["add_file"]["runner"] == "dolphin"
    rpcs3 = groups["RPCS3"]
    assert rpcs3["warning"] == "not found" and rpcs3["off"] is True
    melon = groups["melonDS"]
    assert "(config)" in melon["meta"]
    assert rows_of(form, "melonds")["exe"]["inherited"] is False
    linux = groups["Linux"]
    assert [form.rows[i]["key"] for i in linux["rows"]] == ["add_file"], "the program is the game itself"
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows)))


def test_runners_form_writes_through(api, fake):
    form = api.screens.runners
    form.load()
    batch = next(i for i, r in enumerate(form.rows) if r["module"] == "dolphin" and r["key"] == "batch")
    form.toggle(batch)
    assert form.rows[batch]["value"] is False
    assert fake.runners()[3]["options"][0]["value"] is False
    exe = next(i for i, r in enumerate(form.rows) if r["module"] == "rpcs3" and r["key"] == "exe")
    assert form.setValue(exe, "/opt/rpcs3/rpcs3") is True
    rpcs3 = next(g for g in form.groups if g["title"] == "RPCS3")
    assert rpcs3["warning"] == "" and "/opt/rpcs3/rpcs3 (config)" in rpcs3["meta"]


def test_add_game_flow(api, fake):
    form = api.screens.runners
    form.load()
    messages = []
    form.message.connect(messages.append)
    add = next(i for i, r in enumerate(form.rows) if r["module"] == "dolphin" and r["key"] == "add_file")
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


def test_suggested_titles():
    assert suggested_title("/g/F-Zero GX.iso") == "F-Zero GX"
    assert suggested_title("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci") == "SUPER MARIO ODYSSEY Eur SuperXCi - CLC"
    assert suggested_title("") == ""


def test_game_settings_launch_group_by_runner(api, fake):
    form = api.screens.gameSettings
    form.load("mini-metro")
    assert launch_keys(form) == ["launch.runner", "launch.exe", "launch.runner_exe", "launch.options.fullscreen", "launch.options.inputplumber",
                                 "launch.mangohud", "launch.args", "launch.working_dir"]
    rows = rows_of(form, "")
    assert rows["launch.runner"]["value"] == "Eden" and rows["launch.runner"]["icon"] == "assets/runners/eden.svg"
    assert rows["launch.runner"]["choices"][:4] == ["Proton", "Wine", "Linux", "Dolphin"]
    assert rows["launch.runner"]["choiceValues"][:4] == ["proton", "wine", "linux", "dolphin"]
    assert rows["launch.exe"]["label"] == "File" and rows["launch.exe"]["value"].endswith("Mini Metro.nsp")
    assert rows["launch.runner_exe"]["inherited"] is True and rows["launch.runner_exe"]["value"].endswith("/eden")
    assert rows["launch.options.fullscreen"]["value"] is False and rows["launch.options.fullscreen"]["inherited"] is False
    assert rows["launch.options.inputplumber"]["value"] is True and rows["launch.options.inputplumber"]["inherited"] is True
    assert "launch.proton" not in rows and "platform" not in rows, "one platform: no Platform row"

    form.load("lego-batman")
    rows = rows_of(form, "")
    assert rows["platform"]["type"] == "enum" and rows["platform"]["value"] == "Nintendo Wii"
    assert rows["platform"]["choices"] == ["Nintendo GameCube", "Nintendo Wii"]

    form.load("the-technomancer")
    assert launch_keys(form) == ["launch.runner", "launch.exe", "launch.proton", "launch.esync", "launch.fsync", "launch.prefix",
                                 "launch.mangohud", "launch.args", "launch.working_dir"]
    rows = rows_of(form, "")
    assert rows["launch.runner"]["value"] == "Proton" and rows["launch.exe"]["label"] == "Program"

    index = next(i for i, r in enumerate(form.rows) if r["key"] == "launch.runner")
    assert form.setValue(index, "Dolphin") is True
    assert fake.game("the-technomancer")["launch"]["runner"] == "dolphin"
    assert "platform" in launch_keys(form)
