import contextlib
import importlib
import json
import re
import tomllib
import xml.etree.ElementTree as ET
from collections.abc import Callable
from dataclasses import dataclass, field

import pytest
from _controls import TAKEN, Context, Skip, ini_section, section_values, sections, taken_buttons
from controls_fixtures import BIN_DIR, EDGE, SWITCH_PRO_DIGITAL, script

PADS = [SWITCH_PRO_DIGITAL, EDGE]
SETTINGS = {s["key"]: s for s in tomllib.loads((BIN_DIR.parent / "module.toml").read_text())["settings"]}
UNIVERSE = taken_buttons(EDGE)
RAW = "|".join(map(str, sorted(UNIVERSE)))
NAMED = r"Guide|Misc|Paddle"


def ini(text):
    return {(name, k): v for name in sections(text) for k, v in section_values(text, name).items()}


def tree(node, path=()):
    if not isinstance(node, dict | list):
        return {path: node}
    items = node.items() if isinstance(node, dict) else enumerate(node)
    return {k: v for key, child in items for k, v in tree(child, (*path, key)).items()}


def xml(text):
    def walk(el, path):
        if el.tag == "entry":
            return {(*path, el.findtext("mapping")): el.findtext("button")}
        if not len(el):
            return {(*path, el.tag): el.text}
        return {k: v for child in el for k, v in walk(child, (*path, el.tag)).items()}

    return walk(ET.fromstring(text), ())


def indented(text):
    out, path = {}, []
    for line in text.splitlines():
        key, _, value = line.strip().partition(":")
        path[(len(line) - len(line.lstrip())) // 2 :] = [key]
        if value.strip():
            out[tuple(path)] = value.strip()
    return out


def bound(pattern):
    return lambda text: re.findall(pattern, text, re.MULTILINE)


def both_ways(pairs, prefix=""):
    return {prefix + k: prefix + v for a, b in pairs for k, v in ((a, b), (b, a))}


ABXY = (("a", "b"), ("x", "y"))
L_R = (("l", "zl"), ("r", "zr"))
PS = both_ways((("L1", "L2"), ("R1", "R2")))


@dataclass
class Writer:
    seed: dict[str, str]
    kept: tuple[str, ...]
    read: Callable = ini
    # {binding key: the key whose value it takes under the other choice, None when no single key held it}; None: not offered.
    layout: dict | None = None
    shoulders: dict | None = None
    taken: Callable | None = None
    fresh: bool = False
    ctx: dict = field(default_factory=dict)


P0 = "player_0_button_"
AZ = "profiles\\1\\button_"
WRITERS = {
    "eden": Writer(
        seed={
            "eden/qt-config.ini": '[Controls]\nplayer_0_button_a\\default=true\nplayer_0_button_a="engine:keyboard,code:67,toggle:0"\n'
            "player_0_vibration_enabled=true\n\n[UI]\nShortcuts\\Main%20Window\\Exit%20Eden\\Controller_KeySeq=Home+Minus\n"
        },
        kept=("player_0_vibration_enabled=true", "Controller_KeySeq=Home+Minus"),
        layout=both_ways(ABXY, P0),
        shoulders={**both_ways(L_R, P0), **{P0 + k: P0 + v for k, v in {"slleft": "zl", "srleft": "zr", "slright": "zl", "srright": "zr"}.items()}},
        taken=bound(rf"button:({RAW})\b"),
    ),
    "ryujinx": Writer(
        seed={"Ryujinx/Config.json": json.dumps({"version": 73, "docked_mode": True, "input_config": [{"player_index": "Player1", "deadzone_left": 0.2}]})},
        kept=('"docked_mode": true', '"deadzone_left": 0.2'),
        read=lambda text: tree(json.loads(text)),
        layout=both_ways(ABXY, "button_"),
        shoulders=both_ways(L_R, "button_"),
        taken=bound(NAMED),
    ),
    "dolphin": Writer(
        seed={
            "dolphin-emu/Dolphin.ini": "[Core]\nSIDevice0 = 6\n\n[Interface]\nConfirmStop = False\n",
            "dolphin-emu/GCPadNew.ini": "[GCPad1]\nDevice = SDL/0/DualSense Wireless Controller\nButtons/A = `Button E`\nMain Stick/Calibration = 90.00 120.00\n",
            "dolphin-emu/WiimoteNew.ini": "[Wiimote1]\nSource = 1\nIMUIR/Total Yaw = 50.\n\n[BalanceBoard]\nSource = 0\n",
        },
        kept=("ConfirmStop = False", "Main Stick/Calibration = 90.00 120.00", "IMUIR/Total Yaw = 50.", "[BalanceBoard]"),
        # The GameCube pad's A and B trade places; the remote's B also holds the right trigger.
        layout={**both_ways((("A", "B"), ("X", "Y")), "Classic/Buttons/"), **both_ways((("X", "Y"),), "Buttons/"), "Buttons/A": None, "Buttons/B": None},
        shoulders={
            **dict.fromkeys(("Buttons/Z", "Triggers/L", "Triggers/R", "Triggers/L-Analog", "Triggers/R-Analog", "Buttons/B", "Shake/X", "Shake/Y", "Shake/Z")),
            **both_ways((("Nunchuk/Buttons/C", "Nunchuk/Buttons/Z"),)),
            **both_ways((("Classic/Buttons/ZL", "Classic/Triggers/L"), ("Classic/Buttons/ZR", "Classic/Triggers/R"))),
        },
        taken=bound(NAMED),
    ),
    "cemu": Writer(
        seed={
            "Cemu/controllerProfiles/controller0.xml": '<?xml version="1.0" encoding="UTF-8"?>\n<emulated_controller>\n\t<type>Wii U GamePad</type>\n'
            "\t<controller>\n\t\t<api>SDLController</api>\n\t\t<rumble>0.25</rumble>\n\t</controller>\n</emulated_controller>\n"
        },
        kept=("<rumble>0.25</rumble>",),
        read=xml,
        layout=both_ways((("1", "2"), ("3", "4"))),
        shoulders=both_ways((("5", "7"), ("6", "8"))),
        taken=bound(rf"<button>({'|'.join(map(str, TAKEN))})</button>"),
    ),
    "azahar": Writer(
        seed={
            "azahar-emu/qt-config.ini": '[Controls]\nprofile=0\nprofiles\\1\\button_debug="code:79,engine:keyboard"\nprofiles\\size=1\n\n[UI]\nfullscreen=true\n'
        },
        kept=('profiles\\1\\button_debug="code:79,engine:keyboard"', "fullscreen=true"),
        layout=both_ways(ABXY, AZ),
        shoulders=both_ways(L_R, AZ),
        taken=bound(rf"button:({RAW})\b"),
    ),
    "melonds": Writer(
        seed={
            "melonDS/melonDS.toml": "[Instance0]\nJoystickID = 0\nWindowCount = 1\n\n[Instance0.Keyboard]\nHK_Pause = 32\n\n"
            "[Instance0.Joystick]\nHK_Pause = 5\nHK_Lid = 13\nHK_FastForward = 86048777\n"
        },
        kept=("WindowCount = 1", "[Instance0.Keyboard]", "HK_FastForward = 86048777"),
        layout=both_ways((("A", "B"), ("X", "Y"))),
        taken=lambda text: [v for v in ini_section(text, "Instance0.Joystick").values() if int(v) >= 0 and int(v) & 0xFFFF in UNIVERSE],
    ),
    "mgba": Writer(
        seed={
            "mgba/config.ini": "[gba.input.SDLB]\nkeyA=0\ntiltAxisX=2\n\n[ports.qt]\nfullscreen=1\n",
            "mgba/qt.ini": "[shortcutProfileButton.DualSense%20Edge%20Wireless%20Controller]\nquickSave.1=5\nholdFastForward=12\nsaveState=3\n",
        },
        kept=("tiltAxisX=2", "fullscreen=1", "saveState=3"),
        layout=both_ways((("keyA", "keyB"),)),
        taken=bound(rf"^(?!hat|axis)[\w.]+=({RAW})$"),
    ),
    "snes9x": Writer(
        seed={
            "snes9x/snes9x.conf": "[Shortcuts]\nGTK_quit = Joystick 1 Button 5\nGTK_pause = Joystick 1 Button 16\nQuickSave000 = Keyboard F1\n\n[Display]\nFullscreen = true\n"
        },
        kept=("QuickSave000 = Keyboard F1", "Fullscreen = true"),
        layout=both_ways((("A", "B"), ("X", "Y"))),
        taken=bound(rf"Button ({RAW})\b"),
        fresh=True,
        ctx={"runner_path": "/nix/store/x-snes9x-gtk/bin/snes9x-gtk"},
    ),
    "mupen64plus": Writer(
        seed={"mupen64plus/mupen64plus.cfg": "[Core]\nVersion = 1.010000\n\n[Input-SDL-Control1]\nmode = 2\n\n[Video-General]\nFullscreen = False\n"},
        kept=("Version = 1.010000", "[Video-General]"),
        shoulders={**both_ways((("L Trig", "Z Trig"),)), "R Trig": None},
        taken=bound(rf"button\(({RAW})\)"),
        fresh=True,
    ),
    "rpcs3": Writer(
        seed={
            "rpcs3/input_configs/global/Default.yml": 'Player 1 Input:\n  Handler: DualSense\n  Device: "DualSense Pad #1"\n',
            "rpcs3/input_configs/BLES01807/Default.yml": 'Player 1 Input:\n  Handler: SDL\n  Device: "PS4 Controller 1"\n  Config:\n    Cross: "East"\n'
            'Player 2 Input:\n  Handler: Evdev\n  Device: "Some Evdev Pad"\n',
        },
        kept=('Cross: "East"', 'Device: "Some Evdev Pad"'),
        read=indented,
        shoulders=PS,
        taken=bound(NAMED),
    ),
    "pcsx2": Writer(
        seed={
            "PCSX2/inis/PCSX2.ini": "[UI]\nSettingsVersion = 1\n\n[Pad1]\nType = DualShock2\nDeadzone = 0.1\nMacro1 = SDL-0/Paddle1\n\n"
            "[Hotkeys]\nOpenPauseMenu = SDL-0/Guide\nScreenshot = Keyboard/F8\nQuickSave = SDL-0/Back & SDL-0/Misc1\nToggleTurbo = SDL-0/Back & SDL-0/Start\n"
        },
        kept=("SettingsVersion = 1", "Deadzone = 0.1", "Screenshot = Keyboard/F8", "ToggleTurbo = SDL-0/Back & SDL-0/Start"),
        shoulders=PS,
        taken=bound(NAMED),
    ),
    "duckstation": Writer(
        seed={
            "duckstation/settings.ini": "[Main]\nSettingsVersion = 3\n\n[Pad1]\nType = AnalogController\nAnalogDeadzone = 0.1\nAnalog = SDL-0/Guide\n\n"
            "[Hotkeys]\nOpenPauseMenu = SDL-0/RightPaddle1\nScreenshot = Keyboard/F10\n"
        },
        kept=("SettingsVersion = 3", "AnalogDeadzone = 0.1", "Screenshot = Keyboard/F10"),
        shoulders=PS,
        taken=bound(NAMED),
    ),
    "ppsspp": Writer(
        seed={"ppsspp/PSP/SYSTEM/controls.ini": "﻿[ControlMapping]\nPause = 1-111,10-109\nFast-forward = 1-61,1-59:10-198\n\n[Other]\nKeep = 10-189\n"},
        kept=("﻿[ControlMapping]", "Pause = 1-111", "Keep = 10-189"),
        # HOME (code 4) goes to the PSP's Home: left unbound, it opens PPSSPP's menu.
        taken=bound(r"\b1\d-(19[89]|20[0-3])\b"),
    ),
    "xemu": Writer(
        seed={
            "xemu/xemu/xemu.toml": "[general]\nshow_welcome = false\n\n[input.bindings]\nport1 = 'keyboard'\nport3 = '030000005e040000120b000011050000'\n\n"
            "[sys.files]\nbootrom_path = '/games/xbox/mcpx.bin'\n"
        },
        kept=("show_welcome = false", "bootrom_path = '/games/xbox/mcpx.bin'"),
    ),
    "flycast": Writer(
        seed={"flycast/emu.cfg": "[config]\nrend.Resolution = 960\n\n[input]\ndevice1 = 0\nmaple_sdl_joystick_12 = 1\nmaple_sdl_keyboard = 0\n"},
        kept=("rend.Resolution = 960", "maple_sdl_keyboard = 0"),
    ),
    "scummvm": Writer(
        seed={"scummvm/scummvm.ini": "[scummvm]\nversioninfo=2026.1.0\njoystick_num=0\n\n[monkey2]\ngameid=monkey2\n"},
        kept=("versioninfo=2026.1.0", "gameid=monkey2"),
        fresh=True,
    ),
}


@pytest.fixture
def seeded(request, tmp_path):
    for rel, text in WRITERS[request.param].seed.items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(text)
    return request.param


def plan(name, pads, **settings):
    return importlib.import_module(f"_{name}").plan(Context(pads, **WRITERS[name].ctx, **settings))


def write(planned):
    for path, text in planned.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)


def test_every_writer_the_hook_runs_is_under_contract():
    assert set(WRITERS) == set(script("pre").WRITERS)


@pytest.mark.parametrize("seeded", WRITERS, indirect=True)
def test_planning_over_its_own_output_changes_nothing(seeded):
    write(plan(seeded, PADS))
    assert {path: text for path, text in plan(seeded, PADS).items() if path.read_text() != text} == {}


@pytest.mark.parametrize("seeded", WRITERS, indirect=True)
def test_what_the_writer_does_not_own_survives(seeded, tmp_path):
    write(plan(seeded, PADS))
    after = "".join((tmp_path / rel).read_text() for rel in WRITERS[seeded].seed)
    assert [line for line in WRITERS[seeded].kept if line not in after] == []


@pytest.mark.parametrize(("seeded", "setting"), [(name, setting) for name in WRITERS for setting in ("layout", "shoulders")], indirect=["seeded"])
def test_a_setting_moves_only_the_bindings_it_names(seeded, setting):
    writer, offered = WRITERS[seeded], SETTINGS[setting]
    moves = getattr(writer, setting)
    assert (moves is not None) == (seeded in offered["runners"])
    (other,) = set(offered["choices"]) - {offered["default"]}
    before, after = (
        {(path, *k): v for path, text in plan(seeded, [SWITCH_PRO_DIGITAL], **{setting: choice}).items() for k, v in writer.read(text).items()}
        for choice in (offered["default"], other)
    )
    changed = {k for k in before.keys() | after.keys() if before.get(k) != after.get(k)}
    assert {k[-1] for k in changed} == set(moves or ())
    assert {k: after[k] for k in changed if moves[k[-1]] and after[k] != before[(*k[:-1], moves[k[-1]])]} == {}


@pytest.mark.parametrize("seeded", [name for name, w in WRITERS.items() if w.taken], indirect=True)
def test_universes_buttons_are_never_bound(seeded):
    assert [hit for text in plan(seeded, [EDGE]).values() for hit in WRITERS[seeded].taken(text)] == []


@pytest.mark.parametrize("seeded", WRITERS, indirect=True)
def test_with_no_pad_held_a_writer_skips_or_plans(seeded):
    with contextlib.suppress(Skip):
        plan(seeded, [])


@pytest.mark.parametrize("name", WRITERS)
def test_without_its_config_a_writer_waits_for_the_first_start_or_starts_one(name, tmp_path):
    try:
        planned = set(plan(name, PADS))
    except Skip:
        planned = None
    assert planned == ({tmp_path / next(iter(WRITERS[name].seed))} if WRITERS[name].fresh else None)
