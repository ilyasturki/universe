import os
from collections import Counter

import pytest
from uitest import index_of, own, record, rows_by_key, settle, until

PENDING = {
    "session": "20260912-200000",
    "game": "the-technomancer",
    "state": "pending",
    "started_at": "2026-09-12T20:00:00+02:00",
    "written_at": "",
    "title": "",
    "paragraphs": [],
    "images": [],
}


def test_a_games_cards_show_each_row_once_the_advanced_ones_after_the_rule_unless_the_game_sets_them(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")
    titles = [g["title"] for g in form.groups]

    def cards_hold():
        assert sorted(i for g in form.groups for i in g["rows"]) == [
            i for i, r in enumerate(form.rows) if form.showAdvanced or not r["advanced"] or r.get("changed")
        ], "behind Advanced: the advanced rows the game leaves alone"
        assert [g["title"] for g in form.groups] == titles, "the sidebar does not move with Advanced"
        for g in form.groups:
            advanced = [form.rows[i]["advanced"] for i in g["rows"]]
            assert advanced == sorted(advanced) and g["divider"] == (advanced.index(True) if True in advanced else -1)
            assert g["changed"] == any(form.rows[i].get("changed") for i in g["rows"]), "a card holding a value of the game's own is marked"
        return True

    assert form.hasAdvanced and "advanced" not in rows_by_key(form) and cards_hold(), "a game's page flips Advanced from a button"
    scaler = index_of(form, "launch.gamescope_scaler")
    assert form.setValue(scaler, "fsr") is True and form.rows[scaler]["changed"] and cards_hold(), "a change behind Advanced shows in its card"
    assert form.reset(scaler) is True and not form.rows[scaler]["changed"] and cards_hold()
    assert form.setMapEntry(index_of(form, "launch.env"), "PROTON_SONY_HIDRAW_XINPUT", "1") is True
    assert rows_by_key(form)["launch.env.PROTON_SONY_HIDRAW_XINPUT"]["changed"] and cards_hold(), "the variable shows, the row adding one stays behind"
    form.showAdvanced = True
    assert cards_hold()
    folded = Counter(g["home"] or g["title"] for g in form.advancedGroups)
    assert {g["title"]: len(g["dividers"]) for g in form.groups if g["dividers"]} == folded, "one rule per card folded in"
    assert "codec" not in rows_by_key(form, "capture"), "a module's global settings are not a game's"
    form.load("the-technomancer")
    assert not form.showAdvanced, "Advanced is not remembered across openings"


def test_the_environment_maps_entries_are_rows_of_their_own(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")

    def env():
        return fake.game("the-technomancer")["launch"].get("env", {})

    assert rows_by_key(form, "")["launch.env"]["map"] is True
    assert form.setMapEntry(index_of(form, "launch.env"), "DXVK_HUD", "fps") is True and env() == {"DXVK_HUD": "fps"}
    entry = rows_by_key(form, "")["launch.env.DXVK_HUD"]
    assert (entry["entry"], entry["value"], entry["origin"]) == ("launch.env", "fps", "game") and form.resettable(entry)
    assert form.setMapEntry(index_of(form, "launch.env.DXVK_HUD"), "DXVK_HUD", "") is True and env() == {}, "an emptied value removes it"
    assert form.setMapEntry(index_of(form, "launch.env"), "a.b c", "1") is True and env() == {"abc": "1"}, "a name is one key of the map"
    assert form.reset(index_of(form, "launch.env.abc")) is True and env() == {}
    fake.core.set_setting("launch.env.MANGOHUD", "1")
    form.load("the-technomancer")
    entry = rows_by_key(form, "")["launch.env.MANGOHUD"]
    assert (entry["origin"], entry["inherited"], form.resettable(entry)) == ("global", True, False), "the global's variable, not this game's to drop"
    assert form.setValue(index_of(form, "launch.env.MANGOHUD"), "0") is True and env() == {"MANGOHUD": "0"}
    assert rows_by_key(form, "")["launch.env.MANGOHUD"]["origin"] == "game", "changing an inherited variable writes it on the game"
    assert form.reset(index_of(form, "launch.env.MANGOHUD")) is True and rows_by_key(form, "")["launch.env.MANGOHUD"]["origin"] == "global"


def test_a_module_row_shows_choice_labels_and_sends_the_stored_value(api, fake):
    setting = next(s for m in fake.modules() if m["id"] == "capture" for s in m["settings"] if s["key"] == "source")
    form = api.screens.gameSettings
    form.load("the-technomancer")
    row = rows_by_key(form, "capture")["source"]
    stored = fake.settings("the-technomancer")["capture"]["source"]
    assert row["detail"] == setting["description"]
    assert row["choiceValues"] == ["", *setting["choices"]], "the first choice drops the game's own value"
    assert row["choices"][1:] == [setting["choice_labels"][c] for c in setting["choices"]]
    assert row["display"] == setting["choice_labels"][stored], "the row reads the label of what is stored"
    assert row["value"] == row["choices"][0], "the game sets none: the picker opens on the inherited value"
    other = next(c for c in setting["choices"] if c != stored)
    assert form.setValue(index_of(form, "source"), setting["choice_labels"][other]) is True
    assert fake.settings("the-technomancer")["capture"]["source"] == other, "the picked label is written as its stored value"


@pytest.mark.parametrize(
    ("game", "shown"),
    [
        ("lego-batman", ["enabled", "layout", "wiimote", "shoulders"]),
        ("mini-metro", ["enabled", "layout", "shoulders"]),
        ("the-technomancer", []),
    ],
    ids=["wii", "switch", "proton"],
)
def test_a_module_setting_shows_on_the_games_it_applies_to(api, fake, game, shown):
    form = api.screens.gameSettings
    form.load(game)
    assert list(rows_by_key(form, "controls")) == shown


def test_modules_list(api, fake):
    journal_module = next(m for m in fake.core._data["modules"] if m["id"] == "journal")
    journal_module.update(enabled=False, available=False, missing=["ffmpeg"])
    form = api.screens.modules
    form.load()
    assert [r["module"] for r in form.rows] == ["capture", "journal", "screenshot", "controls"], "the manifests' order, sources apart"
    assert all(r["type"] == "action" and r["key"] == "module" and r["switch"] is True and r["source"] is False for r in form.rows)
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows)))
    assert all(g["off"] is not form.rows[i]["value"] for g in form.groups for i in g["rows"]), "the modules turned off share a card of their own"
    capture, journal = (form.rows[form.indexOf(m)] for m in ("capture", "journal"))
    assert capture["value"] is True and capture["warning"] == ""
    assert journal["value"] is False and "ffmpeg" in journal["warning"]
    next(m for m in fake.core._data["modules"] if m["id"] == "capture").update(available=False, missing=["gsr-cli"])
    form.load()
    capture = form.rows[form.indexOf("capture")]
    assert capture["value"] is True and "gsr-cli" in capture["warning"], "an enabled module with a missing binary does not read as working"
    next(m for m in fake.core._data["modules"] if m["id"] == "capture").update(available=True, missing=[])
    form.load()
    form.toggle(form.indexOf("capture"))
    assert form.rows[form.indexOf("capture")]["value"] is False
    assert next(m for m in fake.modules() if m["id"] == "capture")["enabled"] is False
    form.loadDoctor()
    until(lambda: form.doctor, "the checks run off the UI thread")
    failing = [i for i, r in enumerate(form.doctor) if not r["value"]]
    attention, *cards = form.doctorGroups
    assert attention["rows"] == failing and [form.doctor[i]["module"] for i in failing] == ["core", "runners"], "the failures come first"
    assert all(form.doctor[i]["fix"] and form.doctor[i]["detail"] for i in failing), "a failure says what is wrong and what to do"
    assert sorted(i for g in form.doctorGroups for i in g["rows"]) == list(range(len(form.doctor)))
    assert all(form.doctor[i]["value"] for g in cards for i in g["rows"]), "a card keeps its count but not its failures"
    assert all(len({form.doctor[i]["module"] for i in g["rows"]}) == 1 for g in cards), "a module's or a source's checks share a card"


def test_sources_list(api, fake):
    form = api.screens.sourceList
    form.load()
    until(lambda: form.rows, "the listing probes the logins: off the UI thread")
    assert [r["module"] for r in form.rows] == ["gog", "epic", "itch", "steam"], "the running ones first"
    assert all(r["switch"] is True and r["source"] is True for r in form.rows)
    assert [r["value"] for r in form.rows] == [True, False, False, False]

    def cards_hold():
        assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows))) and all(g["rows"] for g in form.groups), "no empty card"
        assert all(g["off"] is not form.rows[i]["value"] for g in form.groups for i in g["rows"]), "the sources turned off share a card of their own"
        return True

    assert cards_hold() and len(form.groups) == 2
    form.toggle(0)
    until(lambda: [r["value"] for r in form.rows] == [False, False, False, False])
    assert next(s for s in fake.sources() if s["id"] == "gog")["enabled"] is False
    assert cards_hold() and len(form.groups) == 1


def test_source_form(api, fake):
    form = api.screens.source
    form.load("gog")
    until(lambda: form.info)
    assert form.info["source"] is True and form.info["logged_in"] is True and form.info["user"] == "yasso"
    rows = form.rows
    assert rows[0]["key"] == "enabled" and rows[0]["value"] is True and rows[0]["disabled"] is False
    assert sorted(i for g in form.groups for i in g["rows"]) == [i for i, r in enumerate(rows) if not r["advanced"]]

    def card(key):
        return [rows[i]["key"] for i in next(g for g in form.groups if index_of(form, key) in g["rows"])["rows"]]

    assert card("logged_in") == ["logged_in", "link", "code"]
    assert rows[index_of(form, "logged_in")]["type"] == "info" and rows[index_of(form, "logged_in")]["detail"] == "yasso"
    assert card("enabled") == ["enabled", "games_dir", "platform", "with_dlcs"], "every setting: a source's are all global; the advanced ones behind Y"
    assert card("achievements") == ["achievements", "cloud_saves"], "the game settings every game takes, a card of their own"
    assert [rows[i]["key"] for g in form.advancedGroups for i in g["rows"]] == ["scan_dirs", "auth_path", "install_timeout_s"]
    platform = index_of(form, "platform")
    assert rows[platform]["choiceValues"] == ["", "windows", "linux"], "the first choice drops config.toml's own"
    linux = rows[platform]["choices"][2]
    assert form.setValue(platform, linux) is True
    until(lambda: form.rows[index_of(form, "platform")]["value"] == linux)
    assert fake.getSourceSettings("gog")["platform"] == "linux"
    form.toggle(0)
    until(lambda: form.info["enabled"] is False)
    assert [r["key"] for r in form.rows] == ["enabled"], "off: the switch alone, no Advanced row"
    gog = next(s for s in fake.core._data["sources"] if s["id"] == "gog")
    gog.update(available=False, missing=["gogdl"])
    form.load("gog")
    until(lambda: "gogdl" in form.info.get("warning", ""))
    assert form.rows[0]["disabled"] is True
    form.load("capture")
    until(lambda: form.rows == [] and form.info == {}, "a module is not a source")


def test_module_form(api, fake):
    journal_module = next(m for m in fake.core._data["modules"] if m["id"] == "journal")
    journal_module.update(enabled=False, available=False, missing=["ffmpeg"])
    form = api.screens.module
    form.load("journal")
    assert (form.info["name"], form.info["description"]) == (journal_module["name"], journal_module["description"])
    assert "ffmpeg" in form.info["warning"] and form.info["enabled"] is False
    assert [r["key"] for r in form.rows] == ["enabled"], "off: the switch alone"
    assert form.rows[0]["value"] is False and form.rows[0]["disabled"] is True
    assert [(g["rows"], g["meta"], g["warning"]) for g in form.groups] == [([0], "", "")], "the page header carries the name and the warning"
    form.load("capture")
    capture = next(m for m in fake.core._data["modules"] if m["id"] == "capture")
    assert form.info["description"] == capture["description"] and form.info["warning"] == "" and form.info["source"] is False
    rows = form.rows
    assert rows[0]["key"] == "enabled" and rows[0]["value"] is True and rows[0]["disabled"] is False
    settings = next(g for g in form.groups if 0 in g["rows"])
    assert [rows[i]["key"] for i in settings["rows"]] == ["enabled", "codec", "quality", "fps", "size", "audio"], "the switch heads the one card"
    assert [rows[i]["key"] for g in form.advancedGroups for i in g["rows"]] == [
        "container",
        "audio_codec",
        "audio_bitrate",
        "min_duration_s",
        "window_wait_s",
        "ffmpeg_video_opts",
        "gsr_extra_args",
    ], "the advanced settings, then the config-only ones"
    assert form.setValue(form.reveal("gsr_extra_args", "capture"), "-cr full") is True and fake.getSettings("capture", "")["gsr_extra_args"] == "-cr full"
    extra = index_of(form, "gsr_extra_args")
    card = next(g for g in form.groups if extra in g["rows"])
    assert form.showAdvanced and card["rows"].index(extra) >= card["divider"] >= 0, "a write keeps Advanced open, folded into the card"
    form.load("capture")
    assert not form.showAdvanced, "reopening the page does not"
    assert form.setValue(index_of(form, "codec"), "av1") is True
    assert fake.getSettings("capture", "")["codec"] == "av1"
    form.toggle(0)
    assert form.info["enabled"] is False and [r["key"] for r in form.rows] == ["enabled"]
    form.load("nope")
    assert form.rows == [] and form.info == {}
    why = "needs Universe >=9.0.0, this is 0.0.9"
    journal_module.update(missing=[], incompatible=why)
    form.load("journal")
    assert why in form.info["warning"] and "ffmpeg" not in form.info["warning"], "a module for another Universe says which"


def test_module_form_choices(api, fake):
    form = api.screens.module
    form.load("capture")
    rows = rows_by_key(form, "capture")
    assert rows["fps"]["type"] == "int" and rows["fps"]["choiceValues"] == ["", "auto", "120", "90", "60", "30"]
    form.load("journal")
    until(
        lambda: rows_by_key(form, "journal").get("model", {}).get("choices") == ["gpt-6-astra", "gpt-5.6-sol", "gpt-5.5"],
        "the dynamic choices come back from a thread",
    )
    form.load("capture")
    fps = index_of(form, "fps")
    auto = form.rows[fps]["choices"][1]
    assert form.setValue(fps, auto) is True
    assert fake.getSettings("capture", "")["fps"] == "auto"
    assert form.rows[fps]["display"] == auto, "the stored value reads as its label"


def test_launch_form(api, fake):
    form = api.screens.launch
    form.load()
    keys = fake.launchKeys("global", fake.screenMode("DP-1"))
    assert {k["scope"] for k in keys} == {"both", "global"} and "prefix" not in [k["key"] for k in keys]
    titles = [g["title"] for g in form.groups]
    assert not any(form.rows[i]["advanced"] for g in form.groups for i in g["rows"]) and "advanced" not in rows_by_key(form), "beginner first, no Advanced row"
    form.showAdvanced = True
    assert sorted(i for g in form.groups for i in g["rows"]) == list(range(len(form.rows))), "every row sits in one card"
    assert [g["title"] for g in form.groups][: len(titles)] == titles, "Advanced adds cards after the basic ones and moves none"
    rows = rows_by_key(form)
    assert {"launch." + k["key"] for k in keys if not k["runners"]} <= set(rows), "every launch key no runner owns"
    assert not {"launch." + k["key"] for k in keys if k["runners"]} & set(rows), "a runner's keys sit on its page"
    display = next(g for g in form.groups if index_of(form, "launch.gamescope") in g["rows"])
    assert {form.rows[i]["key"] for i in display["rows"]} == {"launch." + k["key"] for k in keys if k["section"] in ("Display", "Scaling")}, (
        "the scaling flags fold into Display"
    )
    assert display["meta"] == form.screen != "", "the Display card names the screen"
    assert rows["paths.games_root"]["value"] == "/mnt/games/PC" and rows["paths.games_root"]["type"] == "path"
    assert rows["keys.sgdb"]["value"] == "" and rows["keys.sgdb"]["secret"] is True
    profile = rows["desktop.profile"]
    assert profile["value"] == profile["choices"][0], "config.toml sets none: the picker opens on the default"
    assert profile["choiceValues"] == ["", "auto", "gnome", "kde", "cinnamon", "sway", "hyprland", "niri", "x11", "none"]
    assert form.setValue(index_of(form, "keys.sgdb"), "abc123") is True and fake.config()["keys"]["sgdb"] == "abc123"
    assert rows_by_key(form)["keys.sgdb"]["value"] == "abc123"
    assert form.setMapEntry(index_of(form, "launch.env"), "MANGOHUD", "1") is True and fake.config()["launch"]["env"] == {"MANGOHUD": "1"}, (
        "a map's entry stays text"
    )
    rows = rows_by_key(form)
    assert rows["launch.env.MANGOHUD"]["value"] == "1" and rows["launch.env.MANGOHUD"]["origin"] == "" and form.resettable(rows["launch.env.MANGOHUD"])
    assert form.setValue(index_of(form, "launch.env.MANGOHUD"), "") is True and "MANGOHUD" not in fake.config()["launch"].get("env", {}), (
        "an emptied value removes the variable"
    )
    assert form.setMapEntry(index_of(form, "launch.env"), "MANGOHUD", "1") is True and form.reset(index_of(form, "launch.env.MANGOHUD")) is True
    assert "MANGOHUD" not in fake.config()["launch"].get("env", {}), "so does X on its row"
    rows = rows_by_key(form)
    assert rows["launch.gamescope"]["detail"] == next(k["description"] for k in keys if k["key"] == "gamescope")
    assert rows["launch.gamescope"]["value"] is True and rows["desktop.hide_cursor"]["value"] is True
    assert rows["launch.gamescope_resolution"]["type"] == "string" and rows["launch.gamescope_resolution"]["value"] == "auto"
    assert rows["launch.gamescope_resolution"]["choices"] == ["auto", "2560x1440", "1920x1080", "1280x720"], (
        "the screen, then the standard heights at its aspect"
    )
    assert rows["launch.gamescope_refresh"]["choices"] == ["auto", "144", "120", "100", "90", "75", "60", "50", "48", "40", "30"]
    assert rows["launch.fps_limit"]["value"] == "auto" and rows["launch.fps_limit"]["choices"] == [
        "auto",
        "none",
        *rows["launch.gamescope_refresh"]["choices"][1:],
    ]
    for key in ("launch.gamescope_scaler", "launch.gamescope_sharpness", "launch.gamescope_adaptive_sync"):
        assert rows[key]["value"] == rows[key]["choices"][0] and rows[key]["choiceValues"][0] == "", f"{key}: the picker opens on the clearing choice"
    assert rows["launch.gamescope_scaler"]["type"] == "enum" and rows["launch.gamescope_scaler"]["choiceValues"] == [
        "",
        "auto",
        "integer",
        "fit",
        "fill",
        "stretch",
    ]
    assert rows["launch.gamescope_args"]["value"] == ""

    index = index_of(form, "launch.gamescope_scaler")
    assert form.setValue(index, "integer") is True
    assert fake.config()["launch"]["gamescope_scaler"] == "integer"
    assert form.setValue(index, rows["launch.gamescope_scaler"]["choices"][0]) is True
    assert "gamescope_scaler" not in fake.config()["launch"], "the clearing choice clears the key"
    assert form.setValue(index_of(form, "launch.gamescope_sharpness"), "7") is True, "a typed value passes through"
    assert fake.config()["launch"]["gamescope_sharpness"] == 7
    form.load()
    assert rows_by_key(form)["launch.gamescope_sharpness"]["value"] == "7"
    form.toggle(index_of(form, "launch.gamescope"))
    assert fake.config()["launch"]["gamescope"] is False
    assert form.setValue(index_of(form, "launch.fps_limit"), "none") is True and fake.config()["launch"]["fps_limit"] == "none"


def test_a_rows_origin_tells_the_games_own_values_and_a_reset_drops_one(api, fake):
    fake.core.set_setting("launch.gamescope_args", "--expose-wayland")
    form = api.screens.gameSettings
    form.load("the-technomancer")
    rows = rows_by_key(form, "")
    assert {k: (rows[k]["origin"], rows[k]["inherited"]) for k in ("launch.proton", "launch.esync", "launch.runner")} == {
        "launch.proton": ("game", False),
        "launch.esync": ("default", True),
        "launch.runner": ("", False),
    }
    assert (rows["launch.gamescope_args"]["origin"], rows["launch.gamescope_args"]["display"]) == ("global", "--expose-wayland"), (
        "the global's arguments show where the game sets none"
    )
    assert rows["launch.working_dir"]["origin"] == "default" and rows["launch.working_dir"]["display"] == os.path.dirname(rows["launch.exe"]["value"]), (
        "an empty working directory shows the program's folder the launch falls back to"
    )
    assert form.reset(index_of(form, "launch.proton")) is True
    assert own(fake, "proton") is None and rows_by_key(form, "")["launch.proton"]["origin"] == "default", (
        "reset drops the game's own value: the row inherits again"
    )
    assert form.reset(index_of(form, "launch.proton")) is False, "nothing of the game's to drop"
    assert form.setValue(index_of(form, "launch.proton"), "proton-ge") is True and rows_by_key(form, "")["launch.proton"]["origin"] == "game", (
        "changing an inherited value is what writes it on the game"
    )
    vrr = index_of(form, "launch.gamescope_adaptive_sync")
    assert form.rows[vrr]["value"] == form.rows[vrr]["choices"][0] and form.rows[vrr]["choiceValues"][0] == "" and form.reset(vrr) is False
    assert form.setValue(vrr, "on") is True and fake.game("the-technomancer")["launch"]["gamescope_adaptive_sync"] == "on"
    assert form.reset(vrr) is True and own(fake, "gamescope_adaptive_sync") is None
    assert form.reset(index_of(form, "launch.runner")) is False, "the runner has no global to go back to"
    capture = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "enabled")
    assert form.rows[capture]["origin"] == "game" and form.reset(capture) is True
    assert "enabled" not in fake.game("the-technomancer")["modules"]["capture"] and form.rows[capture]["origin"] != "game"
    shot = next(i for i, r in enumerate(form.rows) if r["module"] == "screenshot" and r["key"] == "enabled")
    assert form.rows[shot]["origin"] == "default"
    form.toggle(shot)
    assert fake.game("the-technomancer")["modules"]["screenshot"]["enabled"] is False and form.rows[shot]["origin"] == "game", (
        "a toggle writes the inherited value on the game"
    )


def test_game_settings_promote(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")

    def game():
        return fake.game("the-technomancer")

    proton = index_of(form, "launch.proton")
    assert form.setValue(proton, "proton-cachyos") is True
    assert form.promotable(form.rows[proton]) is True and form.promote(proton) is True
    assert fake.config()["set"]["launch"]["proton"] == "proton-cachyos" and own(fake, "proton") is None
    assert rows_by_key(form, "")["launch.proton"]["origin"] == "global", "the game follows the value it handed on"
    assert form.promote(index_of(form, "launch.proton")) is False, "nothing of the game's own left"

    assert form.setMapEntry(index_of(form, "launch.env"), "DXVK_HUD", "fps") is True
    assert form.promote(index_of(form, "launch.env.DXVK_HUD")) is True
    assert fake.config()["set"]["launch"]["env"]["DXVK_HUD"] == "fps" and "DXVK_HUD" not in game()["launch"].get("env", {})

    cursor = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "cursor")
    assert form.promote(cursor) is True and fake.config()["set"]["modules"]["capture"]["cursor"] is False
    assert "cursor" not in game()["modules"]["capture"] and form.rows[cursor]["origin"] == "global"
    capture = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "enabled")
    assert form.rows[capture]["origin"] == "game" and form.promotable(form.rows[capture]) is False, "the module's own switch is another thing"

    assert form.setValue(index_of(form, "sources.gog.achievements"), False) is True
    assert form.promote(index_of(form, "sources.gog.achievements")) is True
    assert fake.config()["set"]["sources"]["gog"]["achievements"] is False and "achievements" not in game().get("sources", {}).get("gog", {})

    assert form.setValue(index_of(form, "launch.working_dir"), "/tmp") is True
    for key in ("launch.runner", "launch.exe", "launch.working_dir", "launch.dll_overrides.d3d11"):
        row = rows_by_key(form).get(key)
        assert row is None or form.promotable(row) is False, f"{key} is the game's alone"

    form.load("mini-metro")
    fullscreen = index_of(form, "launch.options.fullscreen")
    assert form.promote(fullscreen) is True and "fullscreen" not in fake.game("mini-metro")["launch"].get("options", {})
    eden = next(r for r in fake.runners() if r["id"] == "eden")
    assert next(o for o in eden["options"] if o["key"] == "fullscreen")["value"] is False, "a runner option goes to the runner"


def test_a_choice_labelled_value_promoted_reaches_config_as_its_stored_value(api, fake):
    form = api.screens.gameSettings
    form.load("the-technomancer")
    told = record(form.message)
    setting = next(s for m in fake.modules() if m["id"] == "capture" for s in m["settings"] if s["key"] == "source")
    picked = next(c for c in setting["choices"] if c != setting["default"])
    source = next(i for i, r in enumerate(form.rows) if r["module"] == "capture" and r["key"] == "source")
    assert form.setValue(source, setting["choice_labels"][picked]) is True and form.promote(source) is True
    assert fake.config()["set"]["modules"]["capture"]["source"] == picked, "config.toml holds the stored value, not its label"
    row = form.rows[source]
    assert (row["origin"], row["display"], row["changed"]) == ("global", setting["choice_labels"][picked], False), "the game follows it"
    assert len(told) == 1


@pytest.mark.parametrize(
    ("game", "reach"),
    [("lego-batman", "Nintendo Wii games"), ("mini-metro", None), ("the-technomancer", None)],
    ids=["wii", "switch", "proton"],
)
def test_a_value_for_all_games_reaches_the_games_the_setting_applies_to(api, fake, game, reach):
    form = api.screens.gameSettings
    form.load(game)
    wiimote = next((r for r in form.rows if r["module"] == "controls" and r["key"] == "wiimote"), None)
    assert (wiimote and wiimote["reach"]) == reach, "a platform's setting names the platform it reaches"
    if wiimote is None:
        return
    told = record(form.message)
    index = form.rows.index(wiimote)
    assert form.setValueAll(index, wiimote["choices"][2]) is True
    assert fake.config()["set"]["modules"]["controls"]["wiimote"] == "sideways"
    assert "wiimote" not in (fake.game(game).get("modules") or {}).get("controls", {}), "the game follows it"
    assert form.rows[index]["origin"] == "global" and told
    assert form.setValueAll(index_of(form, "launch.exe"), "/x") is False, "a game's program is its alone"
    assert form.toggleAll(index_of(form, "launch.mangohud")) is True and fake.config()["set"]["launch"]["mangohud"] is True
    module = api.screens.module
    module.load("controls")
    defaults = rows_by_key(module, "controls")
    assert defaults["wiimote"]["field"] == "wiimote" and defaults["wiimote"]["resettable"] is True, "the module's page shows the value every game takes"
    assert module.reset(index_of(module, "wiimote")) is True and "wiimote" not in fake.config()["set"]["modules"].get("controls", {}), "and undoes it"


def test_a_games_cards_hold_the_launch_catalogues_sections_and_its_runners(api, fake):
    catalogue = fake.launchKeys("game", fake.screenMode("DP-1"))

    def keys_of(*sections):
        return {"launch." + k["key"] for k in catalogue if k["section"] in sections}

    form = api.screens.gameSettings
    form.load("the-technomancer")
    form.showAdvanced = True

    def card(key):
        return {form.rows[i]["key"] for i in next(g for g in form.groups if index_of(form, key) in g["rows"])["rows"]}

    assert card("launch.gamescope") == keys_of("Display", "Scaling") and card("launch.mangohud") == keys_of("Overlay")
    assert keys_of("Sync", "Upscaling", "Logs") < card("launch.wayland"), "the sync, upscaling and log switches fold into the runner's card"
    form.load("mini-metro")
    assert not {"launch." + k["key"] for k in catalogue if k["runners"]} & set(rows_by_key(form)), "an emulator has no runner card"


def test_sources_browser_flags_each_game_by_where_it_stands(api):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert browser.source == "gog"
    rows = {r["title"]: r for r in browser.rows}
    assert {
        t: (rows[t]["installed"], rows[t]["pending"], rows[t]["partial"]) for t in ("The Technomancer", "Mini Metro", "Stardew Valley", "Disco Elysium")
    } == {
        "The Technomancer": (True, False, False),
        "Mini Metro": (True, True, False),
        "Stardew Valley": (False, False, False),
        "Disco Elysium": (False, False, True),
    }
    assert [r["title"] for r in browser.updates] == ["Mini Metro"]
    assert rows["Dead Cells"]["game_id"] == "dead-cells" and rows["Stardew Valley"]["game_id"] == ""
    assert os.path.isfile(rows["Stardew Valley"]["image"]), "the store's picture, painted locally"
    assert (rows["The Technomancer"]["size"], rows["The Technomancer"]["sizeKind"]) == (8100000000, "disk")
    assert (rows["Stardew Valley"]["size"], rows["Stardew Valley"]["sizeKind"]) == (0, ""), "unknown until peeked"
    assert (rows["Disco Elysium"]["size"], rows["Disco Elysium"]["partial_bytes"]) == (15400000000, 6100000000)
    assert browser.libraryAt == "2026-09-11T19:03:00+02:00"


def test_sources_browser_refresh_hits_the_store_and_page_open_does_not(api, fake):
    core = fake._core
    updates = []
    original = fake.updates
    fake.updates = lambda: updates.append(1) or original()
    browser = api.screens.sources
    browser.load()
    settle(browser)
    browser.load()
    settle(browser)
    assert core.library_calls == [False] and len(updates) == 1, "opening the page serves the cache, and once"
    assert "Alan Wake" not in [r["title"] for r in browser.rows]
    fetched = browser.libraryAt
    browser.refresh()
    settle(browser)
    assert core.library_calls == [False, True] and len(updates) == 2, "Y asks the store"
    assert "Alan Wake" in [r["title"] for r in browser.rows], "a game bought since shows up"
    assert browser.libraryAt != fetched
    browser.uninstall("dead-cells")
    settle(browser)
    assert core.library_calls == [False, True, False], "a reload after a job stays off the network"


def test_sources_browser_free_space_and_a_failed_refresh(api, fake, tmp_path):
    import shutil

    from universe_ui.universe_client import UniverseError

    core = fake._core
    core._data["sources"][0]["games_dir"] = str(tmp_path)
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert browser.error == "" and browser.freeSpace > 0
    assert abs(browser.freeSpace - shutil.disk_usage(tmp_path).free) < 1 << 30, "the install folder's free bytes"
    before = [r["title"] for r in browser.rows]
    library = core.library

    def offline(source, refresh):
        raise UniverseError("Io", "gog library failed: offline")

    core.library = offline
    browser.refresh()
    settle(browser)
    assert browser.error == "gog library failed: offline"
    assert [r["title"] for r in browser.rows] == before, "the listing shown stays"
    core.library = library
    browser.refresh()
    settle(browser)
    assert browser.error == ""


def test_sources_browser_peek_fills_a_size_once(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    rows = {r["title"]: i for i, r in enumerate(browser.rows)}
    browser.peek(rows["The Technomancer"])
    assert fake._core._source_game("gog", "1972906591").get("download_size") is None, "installed rows have their size"
    browser.peek(rows["Stardew Valley"])
    row = until(lambda: (r := browser.rows[rows["Stardew Valley"]])["sizeKind"] == "download" and r)
    assert (row["size"], row["sizeKind"], row["disk_size"]) == (500000000, "download", 1100000000)
    assert not browser.busy, "a peek never shows Loading…"


def test_sources_browser_cancel_pauses_the_install(api, fake, monkeypatch):
    from universe_ui import fake_core

    monkeypatch.setattr(fake_core, "STEP_S", 0.05)
    browser = api.screens.sources
    browser.load()
    settle(browser)
    index = next(i for i, r in enumerate(browser.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    job = browser.install(index)
    assert job and browser.job["game"] == "1207658930"
    rebuilds = record(browser.rowsChanged)
    until(lambda: browser.job["total"] == 50000000000)
    assert browser.rows[index]["busy"]
    assert rebuilds == [], "progress moves the job line, not the rows"
    assert browser.install(index) == "", "one job at a time"
    said = record(browser.message)
    assert browser.cancel() is True and browser.job["cancelled"]
    until(lambda: browser.job["ok"] is not None)
    settle(browser)
    row = browser.rows[index]
    assert browser.job["ok"] is False and said and row["partial"] and row["partial_bytes"] > 0, "stopped: what came down is kept to resume"
    assert browser.cancel() is False, "nothing running"
    browser.install(index)
    until(lambda: browser.job["ok"] is True)
    settle(browser)
    row = next(r for r in browser.rows if r["title"] == "The Witcher 3: Wild Hunt")
    assert row["installed"] and row["size"] == 50000000000
    browser.search("disco")
    settle(browser)
    assert [r["title"] for r in browser.rows] == ["Disco Elysium"]


def test_an_install_arrives_on_home_first_and_lands_as_the_game(api, fake, monkeypatch):
    from universe_ui import fake_core
    from universe_ui.models import HeadedGames, RecentGames

    monkeypatch.setattr(fake_core, "STEP_S", 0.05)
    browser = api.screens.sources
    browser.load()
    settle(browser)
    recent = RecentGames()
    recent.setSourceModel(api.allGames)
    row = HeadedGames()
    row.source = recent
    browser.arrivingChanged.connect(lambda: setattr(row, "head", browser.arriving))
    heads = []
    row.headChanged.connect(lambda: heads.append(row.head.id if row.head else None))
    index = next(i for i, r in enumerate(browser.rows) if r["title"] == "The Witcher 3: Wild Hunt")
    said = record(browser.message)
    assert browser.install(index)
    arriving = browser.arriving
    assert arriving.installing and arriving.title == "The Witcher 3: Wild Hunt" and arriving.progress == -1
    assert row.count == recent.count + 1 and row.get(0) is arriving and row.get(1) is recent.get(0)
    until(lambda: 0 < arriving.progress < 1, "the download's bytes")
    assert browser.cancel() is True
    until(lambda: said)
    settle(browser)
    assert browser.arriving is None and row.count == recent.count and heads == ["arriving:1207658930", None], "stopped: nothing arrives"
    said.clear()
    browser.install(index)
    assert until(lambda: said)[0] == ("Installing 1207658930: done",)
    settle(browser)
    landed = api.allGames.byId("the-witcher-3-wild-hunt")
    assert landed is not None and landed.addedAt is not None and not landed.installing
    assert recent.get(0) is landed and row.get(0) is landed and browser.arriving is None, "the library game takes the first tile"
    assert next(r for r in browser.rows if r["title"] == "The Witcher 3: Wild Hunt")["game_id"] == "the-witcher-3-wild-hunt"


def test_sources_browser_lists_the_store_picked(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    assert [s["id"] for s in browser.stores] == ["gog"] and browser.source == "gog", "a store that is off is not one to list"
    fake.core._source("epic").update(enabled=True, logged_in=True)
    fake.core._data["source_library"]["epic"] = [{"id": "Min", "title": "Hades", "owned": True, "installed": False}]
    browser.refresh()
    settle(browser)
    assert [s["name"] for s in browser.stores] == ["GOG", "Epic Games"] and browser.source == "gog"
    browser.pick("epic")
    settle(browser)
    assert browser.source == "epic" and [r["title"] for r in browser.rows] == ["Hades"] and browser.updates == []
    fake.core._source("epic")["enabled"] = False
    browser.refresh()
    settle(browser)
    assert browser.source == "gog", "the store turned off, the page goes back to one still on"


def test_sources_browser_uninstall_and_remove(api, fake):
    browser = api.screens.sources
    browser.load()
    settle(browser)
    said = record(browser.message)
    browser.uninstall("dead-cells")
    settle(browser)
    row = next(r for r in browser.rows if r["game_id"] == "dead-cells")
    assert len(said) == 1 and not row["installed"], "uninstalled, it stays the library's game"
    browser.uninstall("")
    settle(browser)
    assert len(said) == 1, "a game outside the library has nothing to uninstall"
    browser.uninstall("dead-cells")
    settle(browser)
    assert len(said) == 2, "the core's refusal reaches the toast"
    browser.remove("the-technomancer")
    settle(browser)
    assert len(said) == 3 and fake.game("the-technomancer")["removed"] is True
    browser.remove("no-such-game")
    settle(browser)
    assert len(said) == 4


def test_path_browser(api, tmp_path):
    (tmp_path / "games" / "Mini Metro").mkdir(parents=True)
    (tmp_path / "games" / "Zeta").mkdir()
    (tmp_path / "games" / ".hidden").mkdir()
    (tmp_path / "games" / "notes.txt").write_text("")
    paths = api.screens.paths
    paths.open(str(tmp_path / "games" / "Mini Metro" / "missing"), False)
    assert paths.path == str(tmp_path / "games" / "Mini Metro"), "a gone path opens at its nearest folder"
    assert paths.up() is True
    assert [e["name"] for e in paths.entries] == ["Mini Metro", "Zeta"]
    paths.open(str(tmp_path / "games"), True)
    assert [(e["name"], e["dir"]) for e in paths.entries] == [("Mini Metro", True), ("Zeta", True), ("notes.txt", False)]
    paths.enter(1)
    assert paths.path == str(tmp_path / "games" / "Zeta") and paths.entries == []
    assert paths.shortcuts[0]["path"] == os.path.expanduser("~") and paths.shortcuts[-1]["path"] == "/"
    assert paths.display("/mnt/games") == "/mnt/games", "outside home, verbatim (tmp_path sits under HOME in the nix sandbox)"
    assert paths.display("/") == "/"
    paths.go("/")
    assert paths.atRoot and paths.up() is False


def test_login_flow(api):
    login = api.screens.login
    login.begin("gog")
    assert login.url.startswith("https://")
    assert login.size >= 21 and all(len(row) == login.size for row in login.matrix)
    finished = record(login.finished)
    login.submit("abc")
    assert login.busy
    assert until(lambda: finished)[0][0] is True


def test_removing_a_recording_or_an_entry_reloads_both_lists(api, fake):
    journal, recordings = api.screens.journal, api.screens.recordings
    journal.load("the-technomancer")
    recordings.load("the-technomancer")
    written = [r for r in journal.rows if r["state"] == "written"]
    assert len(written) == 2 and recordings.count == 2
    entry, row = written[0], recordings.rows[0]
    assert entry["paragraphs"] and entry["dateText"] and entry["hasRecording"] is True
    assert row["url"].startswith("file://") and row["size"] == 2147483648 and row["hasJournal"] is True
    assert row["duration_s"] == 4215 and row["gameTitle"] == "The Technomancer"
    assert entry["images"] and all(i.startswith("file://") for i in entry["images"]), "the core hands the images out absolute"
    session = row["session"]

    assert recordings.remove("the-technomancer", session) is True
    assert recordings.count == 1 and session not in recordings.frameMap
    journal.load("the-technomancer")
    assert [r["hasRecording"] for r in journal.rows if r["session"] == session] == [False]

    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))
    assert recordings.remove("the-technomancer", session) is False and errors == ["NotFound"]

    assert journal.remove("the-technomancer", session) is True
    # The session keeps a row of its own, now without an entry: the journal page is where it is written again.
    assert [r["state"] for r in journal.rows if r["session"] == session] == ["none"]


def test_journal_rows_carry_state_and_duration(api, fake):
    entries = fake.core._data["journal"]["the-technomancer"]
    entries.insert(0, dict(PENDING))
    entries.append(
        {
            "session": "20260905-190000",
            "game": "the-technomancer",
            "state": "failed",
            "duration_s": 2520,
            "started_at": "2026-09-05T19:00:00+02:00",
            "written_at": "2026-09-05T19:50:00+02:00",
            "title": "",
            "paragraphs": ["codex timed out after 30 min"],
            "images": [],
        }
    )
    journal = api.screens.journal
    journal.load("the-technomancer")
    rows = journal.rows
    assert [r["state"] for r in rows] == ["pending", "written", "written", "failed", "deferred"]
    pending, first, _, failed, deferred = rows
    assert (pending["title"], pending["reason"], pending["started_at"]) == ("", "", "2026-09-12T20:00:00+02:00")
    assert first["duration_s"] == 4215 and failed["duration_s"] == 2520
    assert failed["reason"] == "codex timed out after 30 min" and failed["blocks"] == ["codex timed out after 30 min"]
    assert deferred["reason"] == "codex quota reached" and deferred["retry_at"] == "2027-01-01T06:00:00+01:00"


def test_pending_journals_announce_each_session_once(api, fake):
    pending = api.screens.pendingJournals
    assert pending.count == 0
    seen = []
    pending.appeared.connect(lambda session, title: seen.append(("appeared", session, title)))
    pending.resolved.connect(lambda session, game, state, text: seen.append(("resolved", session, game, state, text)))
    entries = fake.core._data["journal"]["the-technomancer"]
    entries.insert(0, dict(PENDING))
    fake.entryWritten.emit("20260912-200000", "the-technomancer")
    until(lambda: pending.count == 1, "the read runs off the UI thread")
    assert pending.rows[0]["title"] == "The Technomancer"
    read = record(pending.changed)
    fake.entryWritten.emit("", "the-technomancer")
    fake.sessionEnded.emit("20260912-200000", "the-technomancer", 60, "quit")
    until(lambda: len(read) >= 2, "a read after both signals is in")
    assert seen == [("appeared", "20260912-200000", "The Technomancer")]

    entries[0].update(state="written", title="Back to Noctis", paragraphs=["p"], written_at="2026-09-12T20:50:00+02:00")
    fake.entryWritten.emit("20260912-200000", "the-technomancer")
    until(lambda: pending.count == 0)
    assert seen[-1] == ("resolved", "20260912-200000", "the-technomancer", "written", "Back to Noctis")

    entries.insert(
        0,
        {
            "session": "20260913-100000",
            "game": "the-technomancer",
            "state": "pending",
            "started_at": "2026-09-13T10:00:00+02:00",
            "title": "",
            "paragraphs": [],
            "images": [],
        },
    )
    fake.sessionEnded.emit("20260913-100000", "the-technomancer", 60, "quit")
    until(lambda: pending.count == 1)
    entries[0].update(state="failed", paragraphs=["codex timed out"])
    fake.entryWritten.emit("20260913-100000", "the-technomancer")
    until(lambda: pending.count == 0)
    assert seen[-2:] == [("appeared", "20260913-100000", "The Technomancer"), ("resolved", "20260913-100000", "the-technomancer", "failed", "codex timed out")]


def test_album_and_news_span_every_game(api):
    album = api.screens.album
    album.loadAll()
    assert album.count == 2 and album.gameId == ""
    assert album.rows[0]["gameTitle"] == "The Technomancer" and album.rows[0]["gameId"] == "the-technomancer"
    assert album.rows[0]["created_at"] >= album.rows[1]["created_at"], "newest first"
    news = api.screens.news
    news.loadAll()
    written = [r for r in news.rows if r["state"] == "written"]
    assert len(written) == 2 and written[0]["gameTitle"] == "The Technomancer" and news.gameId == ""
    assert written[0]["written_at"] >= written[1]["written_at"]
    # Every session the library holds is a row, entry or not, newest first.
    assert [r["state"] for r in news.rows] == ["written", "written", "none", "deferred"]


def test_screenshots_list_per_game_and_across_games(api, fake):
    shots = api.screens.shots
    shots.load("the-technomancer")
    assert shots.count > 0 and shots.gameId == "the-technomancer"
    first = shots.rows[0]
    assert first["gameId"] == "the-technomancer" and first["name"].endswith(".png") and first["url"].startswith("file://")
    assert first["session"] != "" and first["hasJournal"] is True, "a shot taken during a journaled session knows its entry"
    assert [r["name"] for r in shots.rows] == sorted((r["name"] for r in shots.rows), reverse=True), "newest first"
    per_game = shots.count
    shots.loadAll()
    assert shots.gameId == "" and shots.count >= per_game and first in shots.rows
    name, ident, before = shots.rows[0]["name"], shots.rows[0]["gameId"], shots.count
    assert shots.remove(ident, name)
    assert shots.count == before - 1 and not any(r["name"] == name and r["gameId"] == ident for r in shots.rows)


def test_media_timeline_merges_the_three_kinds(api):
    media = api.screens.media
    media.load()
    assert media.loading and media.count == 0, "the core's list is read on a worker"
    until(lambda: not media.loading)
    kinds = {r["kind"] for r in media.rows}
    assert kinds == {"shot", "recording", "journal"}
    whens = [r["when"] for r in media.rows]
    assert whens == sorted(whens, reverse=True), "one timeline, newest first"
    shot = next(r for r in media.rows if r["kind"] == "shot")
    assert shot["image"] == "" and shot["thumbReady"] and api.screens.thumbs.url(shot["thumb"]).startswith("file://"), (
        "a card shows the thumbnail, never the original"
    )
    assert shot["url"].startswith("file://") and shot["name"].endswith(".png") and shot["gameTitle"]
    entry = next(r for r in media.rows if r["kind"] == "journal")
    assert entry["title"] and entry["hasJournal"] and entry["session"]
    assert entry["excerpt"] and entry["durationText"], "a journal card shows its words and the session's length"
    rec = next(r for r in media.rows if r["kind"] == "recording")
    assert rec["title"] and rec["session"] and rec["path"] and rec["thumb"] == ""
    assert rec["durationText"] == rec["title"] and shot["durationText"] == "" and shot["excerpt"] == ""


def test_thumbnails_are_announced_as_they_land(api, tmp_path):
    thumbs = api.screens.thumbs
    missing = tmp_path / "shot-1.jpg"
    thumbs.want([str(missing), ""])
    assert thumbs.pending == 1 and thumbs.url(str(missing)) == ""
    version = thumbs.version
    missing.write_bytes(b"jpg")
    until(lambda: thumbs.version != version)
    assert thumbs.version == version + 1 and thumbs.pending == 0 and thumbs.url(str(missing)).startswith("file://")


def test_selecting_a_recording_drops_the_frames_another_was_waiting_for(api, monkeypatch, tmp_path):
    from universe_ui.screens import media

    monkeypatch.setattr(media, "_cache_dir", lambda: str(tmp_path / "frames"))
    recordings = api.screens.recordings
    started = []

    def extract(self, job, frames, vaapi):
        started.append(job)
        self._running[job] = None

    monkeypatch.setattr(media.RecordingsList, "_extract", extract)
    monkeypatch.setattr(media.RecordingsList, "_stop", lambda self, job, proc: self._running.pop(job, None))
    recordings.load("the-technomancer")
    first, second = recordings.rows[0]["session"], recordings.rows[1]["session"]
    recordings._queue.clear()
    recordings._running.clear()
    started.clear()
    recordings.select(first)
    assert started == [(first, 0), (first, 1)] and len(recordings._queue) == 14 and all(j[0] == first for j in recordings._queue), (
        "two at a time, the picked one first"
    )
    started.clear()
    recordings.select(second)
    recordings.select(first)
    assert started == [(second, 0), (second, 1), (first, 0), (first, 1)], "the other session's running frames are stopped, the picked one starts at once"
    others = [j for j in recordings._queue if j[0] != first]
    assert others == [(second, media.THUMB)], "the other session's frames are dropped, its thumbnail stays"
    assert media._ffmpeg_args("/r.mkv", 1.5, "/out.jpg", "/dev/dri/renderD129")[3:9] == [
        "-hwaccel",
        "vaapi",
        "-hwaccel_device",
        "/dev/dri/renderD129",
        "-hwaccel_output_format",
        "vaapi",
    ]
    assert "scale=640:-2" in media._ffmpeg_args("/r.mkv", 1.5, "/out.jpg", None)
    recordings._running.clear()
    recordings._queue.clear()


def test_a_frame_killed_midway_is_not_taken_for_extracted(api, monkeypatch, tmp_path):
    from universe_ui.screens import media

    api.universe.sessions("the-technomancer")
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    ffmpeg = bin_dir / "ffmpeg"
    ffmpeg.write_text('#!/bin/sh\nfor out; do :; done\nprintf "\\377\\330" > "$out"\nexec sleep 30\n')
    ffmpeg.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}:{os.environ['PATH']}")
    monkeypatch.setattr(media, "_cache_dir", lambda: str(tmp_path / "frames"))
    recordings = media.RecordingsList(api.universe)
    recordings._vaapi = None
    recordings.load("the-technomancer")
    session = recordings.rows[0]["session"]
    frames = recordings._frames[session]
    recordings.select(session)
    until(lambda: os.path.exists(frames.partial(media.THUMB)), "ffmpeg has begun the thumbnail")
    recordings.shutdown()

    assert not any(os.path.exists(frames.file(i)) for i in range(media.FRAME_COUNT))
    assert media.Frames(frames.path, frames.duration).extracted == set(), "a later start extracts it again"


def test_journal_paragraphs_become_markdown_blocks():
    from universe_ui.screens.media import markdown_blocks

    assert markdown_blocks(["Intro.", "- **A:** one", "- **B:** two", "Outro.", "1. first", "2. second"]) == [
        "Intro.",
        "- **A:** one\n- **B:** two",
        "Outro.",
        "1. first\n2. second",
    ]
    assert markdown_blocks([]) == []


@pytest.mark.slow
def test_recording_frames_are_sampled_from_the_file(api):
    import shutil

    from universe_ui.screens import media

    if not (shutil.which("ffmpeg") and shutil.which("ffprobe")):
        pytest.skip("ffmpeg and ffprobe sample the frames")
    recordings = api.screens.recordings
    recordings.load("the-technomancer")
    session = recordings.rows[0]["session"]
    recordings.select(session)
    until(lambda: recordings.frameMap[session]["complete"])
    frames = recordings.frameMap[session]
    assert frames["complete"] and all(f.startswith("file://") for f in frames["frames"])
    # The session lasted 1 h 10; the row carries the clip's 20 s, and the seeks follow the file.
    assert 19.5 < frames["duration"] < 20.5
    assert frames["thumbnail"] == frames["frames"][media.THUMB]

    again = media.RecordingsList(api.universe)
    again.load("the-technomancer")
    assert again.frameMap[session]["complete"]
    again.shutdown()


def test_the_sessions_store_lists_played_sessions_and_reads_one_log(api, fake):
    store = api.screens.sessions
    store.load("the-technomancer")
    rows = store.rows
    assert [r["session"] for r in rows] == ["20260909-213045", "20260907-224100", "20260905-190000"], "the Lutris import has no unit, so no log"
    assert [r["end"] for r in rows] == ["quit", "quit", "crashed"]
    assert rows[2]["exit"] == 6 and rows[2]["bad"] and not rows[0]["bad"]
    assert rows[0]["hasRecording"] and not rows[2]["hasRecording"]

    store.openLog("20260907-224100")
    assert store.logLoading
    until(lambda: not store.logLoading)
    log = store.log
    assert store.logSession == "20260907-224100" and not store.logLoading
    assert log[0]["source"] == "universe" and log[0]["message"].startswith("launch 20260907-224100: gamescope")
    assert any(r["error"] and r["source"] == "wine" for r in log)
    assert all(r["time"] == "22:41:00" for r in log)

    store.openLog("20260905-190000")
    until(lambda: not store.logLoading)
    assert store.log[0]["message"].startswith("launch 20260905-190000")

    store.openLog("19700101-000000")
    until(lambda: not store.logLoading)
    assert store.log == [] and "19700101-000000" in store.logError

    launched = record(fake.launched)
    fake.launch("the-technomancer", "")
    until(lambda: launched)
    assert store.rows[0]["live"] and store.rows[0]["session"] == ""
    store.openLog("")
    until(lambda: not store.logLoading)
    assert store.log[0]["message"].startswith("launch " + fake.currentSession["session_id"])
    fake.core.end_session()
    until(lambda: not store.rows[0]["live"])
    assert store.rows[0]["end"] == "quit"
