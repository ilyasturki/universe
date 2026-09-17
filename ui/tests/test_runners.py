from conftest import index_of, rows_by_key
from universe_ui.screens.runners import suggested_title


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
    rows = rows_by_key(form)
    assert rows["exe"]["type"] == "path" and rows["exe"]["inherited"] is True
    assert rows["exe"]["value"].endswith("dolphin-emu") and rows["exe"]["display"] == rows["exe"]["value"], "the found program is the value shown"
    assert rows["exe"]["detail"] == "Found on PATH"
    assert rows["gamescope"]["type"] == "bool" and rows["gamescope"]["inherited"] is True
    assert rows["batch"]["type"] == "bool" and rows["batch"]["value"] is True
    assert rows["add_file"]["type"] == "action" and rows["add_file"]["runner"] == "dolphin"
    form.load("rpcs3")
    assert form.info["warning"] == "not found"
    form.load("melonds")
    assert "(config)" in form.info["meta"] and rows_by_key(form)["exe"]["inherited"] is False
    form.load("linux")
    assert [form.rows[i]["key"] for g in form.groups for i in g["rows"]] == ["gamescope", "add_file"], "the program is the game itself"
    form.load("nope")
    assert form.rows == [] and form.info == {}


def test_runner_form_carries_its_launch_keys(api, fake):
    form = api.screens.runner
    form.load("proton")
    cards = [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups]
    assert cards == [("", ["exe", "args"]), ("", ["gamescope"]), ("Proton", ["launch.proton", "launch.wayland", "launch.hdr"]),
                     ("Sync", ["launch.esync", "launch.fsync", "launch.ntsync"]),
                     ("Upscaling", ["launch.dlss_upgrade", "launch.fsr4_upgrade", "launch.xess_upgrade", "launch.optiscaler"]), ("", ["add_file"])], \
        "config.toml's [launch] keys tied to Proton, the global values"
    groups = {g["title"]: g for g in form.groups}
    assert groups["Upscaling"]["meta"] == "AMD Radeon RX 7900 GRE · RDNA 3" and groups["Upscaling"]["caps"] is True
    rows = rows_by_key(form)
    assert rows["launch.proton"]["value"] == "proton-ge" and rows["launch.proton"]["choices"] == ["proton-cachyos", "proton-em", "proton-ge"]
    assert rows["launch.esync"]["value"] is True and rows["launch.esync"]["inherited"] is False
    assert rows["launch.dlss_upgrade"]["detail"].endswith("NVIDIA GeForce RTX only. Not for your GPU.")
    assert rows["launch.fsr4_upgrade"]["detail"].endswith("Works on your GPU.") and rows["launch.optiscaler"]["detail"].endswith("Works on your GPU.")
    form.toggle(index_of(form, "launch.fsr4_upgrade"))
    assert fake.config()["launch"]["fsr4_upgrade"] is True and rows_by_key(form)["launch.fsr4_upgrade"]["value"] is True
    assert form.setValue(index_of(form, "launch.proton"), "proton-em") is True and fake.config()["launch"]["proton"] == "proton-em"
    form.load("wine")
    assert [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups][2] == ("Sync", ["launch.esync", "launch.fsync"]), "no NTSync, no Proton build on plain Wine"
    assert [g["title"] for g in form.groups] == ["", "", "Sync", ""]
    form.load("dolphin")
    assert "launch.esync" not in rows_by_key(form)
    fake.core._data["gpu"] = None
    form.load("proton")
    assert {g["title"]: g["meta"] for g in form.groups}["Upscaling"] == "" and rows_by_key(form)["launch.dlss_upgrade"]["detail"].endswith("RTX only.")


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
                                 "launch.wrapper", "launch.args", "launch.working_dir"]
    rows = rows_by_key(form)
    assert rows["launch.runner"]["value"] == "Eden" and rows["launch.runner"]["icon"] == "assets/runners/eden.svg"
    assert rows["launch.runner"]["choices"][:4] == ["Proton", "Wine", "Linux", "Dolphin"]
    assert rows["launch.runner"]["choiceValues"][:4] == ["proton", "wine", "linux", "dolphin"]
    assert rows["launch.exe"]["label"] == "File" and rows["launch.exe"]["value"].endswith("Mini Metro.nsp")
    assert rows["launch.runner_exe"]["inherited"] is True and rows["launch.runner_exe"]["value"].endswith("/eden")
    assert rows["launch.options.fullscreen"]["value"] is False and rows["launch.options.fullscreen"]["inherited"] is False
    assert rows["launch.options.inputplumber"]["value"] is True and rows["launch.options.inputplumber"]["inherited"] is True
    assert "launch.proton" not in rows and "platform" not in rows, "one platform: no Platform row"

    form.load("lego-batman")
    rows = rows_by_key(form)
    assert rows["platform"]["type"] == "enum" and rows["platform"]["value"] == "Nintendo Wii"
    assert rows["platform"]["choices"] == ["Nintendo GameCube", "Nintendo Wii"]

    form.load("the-technomancer")
    assert launch_keys(form) == ["launch.runner", "launch.exe", "launch.wrapper", "launch.args", "launch.working_dir"]
    assert [(g["title"], [form.rows[i]["key"] for i in g["rows"]]) for g in form.groups if g["title"] in ("Proton", "Sync", "Upscaling")] == [
        ("Proton", ["launch.proton", "launch.wayland", "launch.hdr", "launch.prefix"]), ("Sync", ["launch.esync", "launch.fsync", "launch.ntsync"]),
        ("Upscaling", ["launch.dlss_upgrade", "launch.fsr4_upgrade", "launch.xess_upgrade", "launch.optiscaler"])]
    rows = rows_by_key(form)
    assert rows["launch.runner"]["value"] == "Proton" and rows["launch.exe"]["label"] == "Program"
    assert rows["launch.wayland"]["value"] is True and rows["launch.wayland"]["inherited"] is True
    assert rows["launch.hdr"]["value"] is False and rows["launch.hdr"]["inherited"] is True

    index = index_of(form, "launch.runner")
    assert form.setValue(index, "Dolphin") is True
    assert fake.game("the-technomancer")["launch"]["runner"] == "dolphin"
    assert "platform" in launch_keys(form)
