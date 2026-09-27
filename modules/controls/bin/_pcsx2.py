import itertools
import os
import re
import shutil
from pathlib import Path

from _controls import Context, Skip, config_home, edit_section, first_file, key_of, section_values, sections, value_of

KEYS = {
    "Up": "DPadUp",
    "Right": "DPadRight",
    "Down": "DPadDown",
    "Left": "DPadLeft",
    "Select": "Back",
    "Start": "Start",
    "L1": "LeftShoulder",
    "R1": "RightShoulder",
    "L2": "+LeftTrigger",
    "R2": "+RightTrigger",
    "L3": "LeftStick",
    "R3": "RightStick",
    "LUp": "-LeftY",
    "LRight": "+LeftX",
    "LDown": "+LeftY",
    "LLeft": "-LeftX",
    "RUp": "-RightY",
    "RRight": "+RightX",
    "RDown": "+RightY",
    "RLeft": "-RightX",
    "LargeMotor": "LargeMotor",
    "SmallMotor": "SmallMotor",
}
# A/B/X/Y are SDL2's label names, which PCSX2 migrates on load: the positional ones are what it writes itself.
FACE = {"Cross": "FaceSouth", "Circle": "FaceEast", "Square": "FaceWest", "Triangle": "FaceNorth"}
TAKEN = ("Misc1", "Misc2", "Misc3", "Misc4", "Misc5", "Misc6", "Paddle1", "Paddle2", "Paddle3", "Paddle4")


def takes_universe(value, pattern):
    return any(pattern.match(part.strip()) for part in value.split("&"))


def player_ids(pads):
    used, out = set(), []
    for pad in pads:
        pid = pad.player_index if pad.player_index >= 0 and pad.player_index not in used else next(i for i in itertools.count() if i not in used)
        used.add(pid)
        out.append(pid)
    return out


def bind(lines, values, pad_type, pattern):
    connected = any(key_of(line) == "Type" and value_of(line) != "None" for line in lines)

    def keep(line):
        key = key_of(line)
        if key is None:
            return True
        if key in values or (key == "Type" and not connected):
            return False
        return not takes_universe(value_of(line), pattern)

    return [*filter(keep, lines), *([] if connected else [f"Type = {pad_type}"]), *(f"{k} = {v}" for k, v in values.items())]


def write_pads(path, slots, ctx: Context, *, name, face, taken, pad_type):
    if path is None:
        raise Skip(f"no {name} settings yet: start {name} once, then its controls are written")
    text = path.read_text()
    names = [*taken, *([] if ctx.guide else ["Guide"])]
    pattern = re.compile(rf"^SDL-[^/]+/({'|'.join(map(re.escape, names))})$")
    for slot, pid in zip(slots(text), player_ids(ctx.pads), strict=False):
        values = {k: f"SDL-{pid}/{v}" for k, v in {**face, **KEYS}.items()}
        if ctx.guide:
            values["Analog"] = f"SDL-{pid}/Guide"
        text = edit_section(text, slot, lambda lines, v=values: bind(lines, v, pad_type, pattern))
    for section in sections(text):
        if section == "Hotkeys" or re.fullmatch(r"Pad[1-8]", section):
            text = edit_section(text, section, lambda lines: [line for line in lines if key_of(line) is None or not takes_universe(value_of(line), pattern)])
    return {path: text}


def extra_hints(text):
    return {k: v for k, v in section_values(text, "SDLHints").items() if k.startswith("SDL_")}


def prefix(names):
    for name in names:
        if found := shutil.which(name):
            return Path(os.path.realpath(found)).parent.parent
    return None


def data_root():
    return config_home() / "PCSX2"


def config_path():
    return first_file(data_root() / "inis" / "PCSX2.ini")


def pad_slots(port1, port2):
    return ["Pad1", "Pad2", *(["Pad3", "Pad4", "Pad5"] if port1 else []), *(["Pad6", "Pad7", "Pad8"] if port2 else [])]


def slots(text):
    pad = section_values(text, "Pad")
    return pad_slots(pad.get("MultitapPort1") == "true", pad.get("MultitapPort2") == "true")


def databases():
    bundled = prefix(("pcsx2-qt", "pcsx2"))
    found = first_file(
        data_root() / "game_controller_db.txt",
        data_root() / "resources" / "game_controller_db.txt",
        *([bundled / "share" / "PCSX2" / "resources" / "game_controller_db.txt"] if bundled else []),
    )
    return [found] if found else []


def hints():
    path = config_path()
    text = path.read_text(errors="replace") if path else ""
    out = {
        "SDL_JOYSTICK_HIDAPI_PS3": "1",
        "SDL_JOYSTICK_HIDAPI_WII": "1",
        "SDL_JOYSTICK_ENHANCED_REPORTS": "0" if section_values(text, "InputSources").get("SDLControllerEnhancedMode") == "false" else "auto",
    }
    return {**out, **extra_hints(text)}


def plan(ctx: Context):
    return write_pads(config_path(), slots, ctx, name="PCSX2", face=FACE, taken=TAKEN, pad_type="DualShock2")
