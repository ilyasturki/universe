import json
import os
import subprocess
from pathlib import Path

import pytest
from _controls import ini_section
from controls_fixtures import CORPUS, EDGE, PRO3_DINPUT

MODULE = Path(__file__).resolve().parents[1]
SESSION = "20261006-213000"
# The user's library: (file, runner, platform, the game's own controls settings). config.toml swaps the shoulders for all.
GAMES = {
    "mkwii": ("Mario Kart Wii.wbfs", "dolphin", "Nintendo Wii", ()),
    "nsmbw": ("New Super Mario Bros. Wii.wbfs", "dolphin", "Nintendo Wii", ("wiimote=sideways", "shoulders=standard")),
    "xenoblade": ("Xenoblade Chronicles.wbfs", "dolphin", "Nintendo Wii", ("wiimote=classic",)),
    "double-dash": ("Mario Kart Double Dash.rvz", "dolphin", "Nintendo GameCube", ()),
    "mk8": ("Mario Kart 8 Deluxe.nsp", "eden", "", ()),
    "demons-souls": ("Demons Souls.pkg", "rpcs3", "", ()),
    "bloodborne": ("Bloodborne.pkg", "shadps4", "", ()),
    "halo-3": ("Halo 3.iso", "xenia", "", ()),
    "hades": ("Hades.exe", "proton", "", ()),
}
# The printed letter of each face button: A on the right, B at the bottom, X at the top, Y on the left.
PRINTED = {"`Button E`": "A", "`Button S`": "B", "`Button N`": "X", "`Button W`": "Y"}
# Eden binds SDL's raw buttons 0-3: SDL's 8BitDo driver numbers them by printed letter, xpad and the PS5 driver by position.
RAW = {"PRO3_DINPUT": "ABXY", "PRO3_XINPUT": "BAYX", "EDGE": "BAYX"}
# The layout table: the printed button each console button lands on.
NINTENDO = {"positional": {"A": "A", "B": "B", "X": "X", "Y": "Y"}, "xbox": {"A": "B", "B": "A", "X": "Y", "Y": "X"}}
NUNCHUK = {"positional": {"A": "A", "B": "B", "2": "X", "1": "Y"}, "xbox": {"A": "B", "B": "A", "2": "X", "1": "Y"}}
SIDEWAYS = {"positional": {"2": "A", "1": "B", "A": "X", "B": "Y"}, "xbox": {"1": "A", "2": "B", "B": "X", "A": "Y"}}
SDL_NAME = {"PRO3_DINPUT": "8BitDo Pro 3", "PRO3_XINPUT": "Xbox 360 Controller"}
EDGE_NAME = "DualSense Edge Wireless Controller"
XINPUT_DEVICE = "/sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.0"


@pytest.fixture(scope="module")
def universe(tmp_path_factory):
    binary = os.environ.get("UNIVERSE_BIN")
    if not binary:
        pytest.fail("UNIVERSE_BIN names the universe build the launch runs on (`just test` sets it)")
    root = tmp_path_factory.mktemp("profile")
    env = {
        **os.environ,
        "HOME": str(root),
        "XDG_CONFIG_HOME": str(root / ".config"),
        "XDG_DATA_HOME": str(root / ".local/share"),
        "UNIVERSE_MODULES_PATH": str(MODULE.parent),
        "UNIVERSE_SOURCES_PATH": str(root / "sources"),
        "UNIVERSE_BIN": binary,
        **{f"UNIVERSE_{part}_HOME": str(root / part.lower()) for part in ("DATA", "CONFIG", "STATE", "CACHE")},
    }

    def run(*args):
        out = subprocess.run([binary, *args], env=env, capture_output=True, text=True, check=False)
        assert out.returncode == 0, f"universe {' '.join(args)}: {out.stderr}"
        return out.stdout

    (root / "roms").mkdir()
    run("module", "set", "controls", "shoulders=swapped")
    games = {}
    for key, (file, runner, platform, settings) in GAMES.items():
        (root / "roms" / file).touch()
        added = json.loads(run("--json", "add", str(root / "roms" / file), "--runner", runner, *(("--platform", platform) if platform else ())))
        if settings:
            run("module", "set", "controls", *settings, "--game", added["id"])
        games[key] = run("--json", "info", added["id"])
    run.env, run.games = env, games
    return run


@pytest.fixture
def launch(universe, tmp_path):
    """`pads` is SDL's list, which launch/sitecustomize.py serves the hook."""
    config = tmp_path / ".config"
    (config / "dolphin-emu").mkdir(parents=True)
    (config / "dolphin-emu" / "Dolphin.ini").write_text("[Core]\nSIDevice0 = 6\n")
    (config / "eden").mkdir()
    (config / "eden" / "qt-config.ini").write_text("[Controls]\n")
    state = Path(universe.env["UNIVERSE_STATE_HOME"])
    state.mkdir(exist_ok=True)

    def run(game, pads, devices=(), mapped=True, **extra):
        info = universe.games[game]
        g = json.loads(info)
        env_file = state / f"env-{SESSION}"
        env_file.write_text("")
        env = {
            **universe.env,
            "HOME": str(tmp_path),
            "XDG_CONFIG_HOME": str(config),
            "XDG_DATA_HOME": str(tmp_path / ".local/share"),
            "XDG_RUNTIME_DIR": str(tmp_path / "run"),
            "GAME_ID": g["id"],
            "GAME_SLUG": g["id"],
            "GAME_TITLE": g["title"],
            "GAME_DIR": g["dir"],
            "UNIVERSE_GAME_JSON": info,
            "MODULE_SETTINGS_JSON": universe("--json", "module", "settings", "controls", g["id"]),
            "SESSION_ID": SESSION,
            "SESSION_SCREEN": "",
            "UNIVERSE_ENV_FILE": str(env_file),
            "MODULE_DIR": str(MODULE),
            "MODULE_DATA_DIR": str(Path(universe.env["UNIVERSE_DATA_HOME"]) / "modules" / "controls"),
            "PYTHONPATH": str(Path(__file__).resolve().parent / "launch"),
            "CONTROLS_PADS": ",".join(pads),
            "CONTROLS_DEVICES": json.dumps(list(devices)),
            "CONTROLS_HINTS": str(tmp_path / "hints.json"),
            **extra,
        }
        out = subprocess.run([MODULE / "bin" / "pre"], cwd=MODULE, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=10, check=False)
        assert out.returncode == 0, out.stderr
        if mapped:
            assert env_file.read_text() == f"SDL_GAMECONTROLLERCONFIG_FILE={run.mapping}\n"
        return env_file.read_text()

    def ini(*path):
        return (config / Path(*path)).read_text()

    run.ini, run.runtime, run.config = ini, tmp_path / "run" / "universe", config
    run.mapping, run.hints = run.runtime / f"controls-{SESSION}.txt", tmp_path / "hints.json"
    return run


def face(binding):
    """The printed letter a binding names, and what else it is bound to."""
    parts = [p.strip() for p in binding.split("|")]
    letters = [PRINTED[p] for p in parts if p in PRINTED]
    assert len(letters) == 1, binding
    return letters[0], {p for p in parts if p not in PRINTED}


def faces(section, prefix, buttons):
    return {b: face(section[f"{prefix}{b}"])[0] for b in buttons}


def eden_faces(controls, raw, player=0):
    return {b.upper(): raw[int(controls[f"player_{player}_button_{b}"].rsplit("button:", 1)[1])] for b in "abxy"}


@pytest.mark.parametrize("layout", ["positional", "xbox"])
@pytest.mark.parametrize("pad", ["PRO3_DINPUT", "PRO3_XINPUT"])
def test_a_pro_3_launched_lands_each_nintendo_button_where_the_layout_puts_it(universe, launch, pad, layout):
    universe("module", "set", "controls", f"layout={layout}")

    launch("double-dash", [pad])
    gcpad = ini_section(launch.ini("dolphin-emu", "GCPadNew.ini"), "GCPad1")
    assert gcpad["Device"] == f"SDL/0/{SDL_NAME[pad]}"
    assert faces(gcpad, "Buttons/", "ABXY") == NINTENDO[layout]
    assert (gcpad["Triggers/L"], gcpad["Triggers/R"]) == ("`Shoulder L`", "`Shoulder R`"), "swapped: L and R on the bumpers"
    assert set(gcpad["Buttons/Z"].split(" | ")) == {"`Trigger L`", "`Trigger R`"}, "and Z on either trigger"
    assert ini_section(launch.ini("dolphin-emu", "Dolphin.ini"), "Core")["SIDevice0"] == "6"

    launch("mkwii", [pad])
    remote = ini_section(launch.ini("dolphin-emu", "WiimoteNew.ini"), "Wiimote1")
    assert (remote["Extension"], remote["Options/Sideways Wiimote"]) == ("Nunchuk", "False")
    assert faces(remote, "Buttons/", "AB12") == NUNCHUK[layout]
    assert face(remote["Buttons/B"])[1] == {"`Shoulder R`"}, "swapped: the remote's B also on the right bumper"
    nunchuk = (remote["Nunchuk/Buttons/Z"], remote["Nunchuk/Buttons/C"], remote["Shake/X"])
    assert nunchuk == ("`Shoulder L`", "`Trigger L`", "`Trigger R`"), "Z on the left bumper, C on the left trigger, the shake on the right trigger"

    launch("nsmbw", [pad])
    sideways = ini_section(launch.ini("dolphin-emu", "WiimoteNew.ini"), "Wiimote1")
    assert (sideways["Extension"], sideways["Options/Sideways Wiimote"]) == ("None", "True")
    assert faces(sideways, "Buttons/", "AB12") == SIDEWAYS[layout]
    assert face(sideways["Buttons/B"])[1] == {"`Trigger R`"}, "the remote's B also on the right trigger"

    launch("xenoblade", [pad])
    classic = ini_section(launch.ini("dolphin-emu", "WiimoteNew.ini"), "Wiimote1")
    assert classic["Extension"] == "Classic"
    assert faces(classic, "Classic/Buttons/", "ABXY") == NINTENDO[layout]

    launch("mk8", [pad])
    assert eden_faces(ini_section(launch.ini("eden", "qt-config.ini"), "Controls"), RAW[pad]) == NINTENDO[layout]


@pytest.mark.parametrize("pad", ["PRO3_DINPUT", f"PRO3_XINPUT={XINPUT_DEVICE}"])
def test_the_pad_last_pressed_is_player_one_with_an_edge_connected_first(universe, launch, pad):
    name, _, device = pad.partition("=")
    launch("mkwii", ["EDGE", pad])
    assert ini_section(launch.ini("dolphin-emu", "GCPadNew.ini"), "GCPad1")["Device"] == f"SDL/0/{EDGE_NAME}", "nothing pressed yet: SDL's order"

    launch.runtime.mkdir(parents=True, exist_ok=True)
    (launch.runtime / "active-pad").write_text((device or PRO3_DINPUT.device) + "\n")
    launch("mkwii", ["EDGE", pad])
    remotes = launch.ini("dolphin-emu", "WiimoteNew.ini")
    assert [ini_section(remotes, f"Wiimote{n}")["Device"] for n in (1, 2)] == [f"SDL/0/{SDL_NAME[name]}", f"SDL/0/{EDGE_NAME}"]
    assert ini_section(launch.ini("dolphin-emu", "GCPadNew.ini"), "GCPad1")["Device"] == f"SDL/0/{SDL_NAME[name]}"
    launch("mk8", ["EDGE", pad])
    controls = ini_section(launch.ini("eden", "qt-config.ini"), "Controls")
    assert [controls[f"player_{n}_button_a"].split("guid:")[1][8:12] for n in (0, 1)] == [{"PRO3_DINPUT": "c82d", "PRO3_XINPUT": "5e04"}[name], "4c05"]


# The Pro 3's device as `controller ls` lists it, with its model's `[controller.layout]` choice.
def pro3_device(layout):
    return {"id": "event99", "family": "8bitdo-pro-3", "vendor": 0x2DC8, "product": 0x6009, "path": "/dev/input/event99", "layout": layout}


@pytest.mark.parametrize("override", ["", "xbox"])
@pytest.mark.parametrize("layout", ["pad", "positional", "xbox"])
def test_each_pad_held_gets_its_own_face_unless_the_game_names_one(universe, launch, layout, override):
    universe("module", "set", "controls", f"layout={layout}")
    held = ["EDGE", "PRO3_DINPUT"]
    # On `pad`, Cross counts as A, so the Edge's A sits at the bottom; the Pro 3's A is printed on the right, or where its model is set.
    expected = {"EDGE": "xbox", "PRO3_DINPUT": override or "positional"} if layout == "pad" else dict.fromkeys(held, layout)

    launch("double-dash", held, [pro3_device(override)])
    gcpads = launch.ini("dolphin-emu", "GCPadNew.ini")
    assert [faces(ini_section(gcpads, f"GCPad{n}"), "Buttons/", "ABXY") for n in (1, 2)] == [NINTENDO[expected[p]] for p in held]

    launch("mk8", held, [pro3_device(override)])
    controls = ini_section(launch.ini("eden", "qt-config.ini"), "Controls")
    assert [eden_faces(controls, RAW[p], n) for n, p in enumerate(held)] == [NINTENDO[expected[p]] for p in held]


# `universe controller dump` records what this hook prints of each pad SDL3 lists: the format tests/pads keeps.
def test_the_pads_hook_prints_sdls_view_as_the_recorded_pads_keep_it():
    env = {**os.environ, "PYTHONPATH": str(Path(__file__).resolve().parent / "launch"), "CONTROLS_PADS": "EDGE,PRO3_DINPUT"}
    out = subprocess.run([MODULE / "bin" / "pads"], cwd=MODULE, env=env, capture_output=True, text=True, timeout=10, check=True)
    lines = [json.loads(line) for line in out.stdout.splitlines()]
    assert [(line["device"], line["vendor"], line["product"]) for line in lines] == [(EDGE.device, 0x054C, 0x0DF2), (PRO3_DINPUT.device, 0x2DC8, 0x6009)]
    assert [line["sdl"] for line in lines] == [
        json.loads((CORPUS / f"{name}.json").read_text())["sdl"] for name in ("dualsense-edge-bt", "8bitdo-pro-3-dinput-bt")
    ]


def test_a_config_is_written_through_its_symlink_and_backed_up_once(launch, tmp_path):
    own = '[Controls]\nplayer_0_button_a="engine:keyboard,code:67,toggle:0"\n'
    kept = tmp_path / "dotfiles" / "qt-config.ini"
    kept.parent.mkdir()
    kept.write_text(own)
    ini = launch.config / "eden" / "qt-config.ini"
    ini.unlink()
    ini.symlink_to(kept)
    launch("mk8", ["EDGE"])
    assert ini.is_symlink() and "player_0_connected=true" in kept.read_text(), "the dotfiles' copy is written, the link kept"
    launch("mk8", ["PRO3_DINPUT"])
    assert kept.with_name("qt-config.ini.before-universe").read_text() == own, "the user's own file, backed up the first time only"


def test_an_emulator_with_nothing_to_write_still_gets_the_pads_without_universes_buttons(launch):
    launch("bloodborne", ["EDGE"])
    line = launch.mapping.read_text().strip()
    assert line.startswith(EDGE.guid.hex()) and "guide" not in line and "paddle1" not in line


def test_an_emulators_own_pad_database_goes_to_sdl_and_under_the_mapping(launch):
    db = launch.config / "rpcs3" / "input_configs" / "gamecontrollerdb.txt"
    db.parent.mkdir(parents=True)
    db.write_text("# rpcs3\n")
    launch("demons-souls", ["EDGE"])
    assert json.loads(launch.hints.read_text()) == {"SDL_GAMECONTROLLERCONFIG_FILE": str(db), "SDL_JOYSTICK_HIDAPI_PS3": "1"}
    assert launch.mapping.read_text().startswith("# rpcs3\n")


@pytest.mark.parametrize("game", ["hades", "halo-3"], ids=["proton", "xenia-through-proton"])
def test_the_hook_leaves_a_game_alone_where_it_does_not_apply(launch, game):
    assert launch(game, ["EDGE"], mapped=False) == ""
    assert not launch.mapping.exists() and launch.ini("eden", "qt-config.ini") == "[Controls]\n"


def test_the_hook_stands_down_without_pads_and_for_the_pads_module(universe, launch, tmp_path):
    assert launch("mk8", [], mapped=False) == "" and launch.ini("eden", "qt-config.ini") == "[Controls]\n", "no pad held"
    shims = tmp_path / "bin"
    shims.mkdir()
    for tool in ("systemd-run", "systemctl"):
        (shims / tool).write_text("#!/bin/sh\n")
        (shims / tool).chmod(0o755)
    uhid = tmp_path / "uhid"
    uhid.touch()
    universe("module", "enable", "pads")
    try:
        env = {"PATH": f"{shims}{os.pathsep}{os.environ['PATH']}", "UNIVERSE_UHID": str(uhid)}
        assert launch("mk8", ["EDGE"], mapped=False, **env) == "", "the pads module hands the emulator its own pads"
        assert launch.ini("eden", "qt-config.ini") == "[Controls]\n"
        universe("module", "set", "pads", "enabled=false", "--game", json.loads(universe.games["mk8"])["id"])
        launch("mk8", ["EDGE"], **env)
        assert "player_0_connected=true" in launch.ini("eden", "qt-config.ini"), "turned off for the game: the controls are written"
    finally:
        universe("module", "disable", "pads")
