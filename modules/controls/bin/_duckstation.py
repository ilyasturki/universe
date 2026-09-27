import os
from pathlib import Path

from _controls import Context, first_file, section_values
from _pcsx2 import extra_hints, pad_slots, write_pads

# SDL3's own names, positional: A is SOUTH on every pad.
FACE = {"Cross": "A", "Circle": "B", "Square": "X", "Triangle": "Y"}
TAKEN = ("Misc1", "Misc2", "Misc3", "Misc4", "Misc5", "Misc6", "RightPaddle1", "LeftPaddle1", "RightPaddle2", "LeftPaddle2")


def data_roots(runner_path=""):
    xdg = os.environ.get("XDG_CONFIG_HOME", "")
    home = Path.home()
    return [
        *([Path(os.path.realpath(runner_path)).parent] if runner_path else []),
        *([Path(xdg) / "duckstation"] if os.path.isabs(xdg) else []),
        home / ".local/share/duckstation",
        home / ".var/app/org.duckstation.DuckStation/config/duckstation",
    ]


def config_path(runner_path=""):
    return first_file(*(root / "settings.ini" for root in data_roots(runner_path)))


def slots(text):
    mode = section_values(text, "ControllerPorts").get("MultitapMode", "Disabled")
    return pad_slots(mode in ("Port1Only", "BothPorts"), mode in ("Port2Only", "BothPorts"))


def databases():
    path = config_path()
    found = path and first_file(path.parent / "gamecontrollerdb.txt")
    return [found] if found else []


def hints():
    path = config_path()
    text = path.read_text(errors="replace") if path else ""
    sources = section_values(text, "InputSources")
    out = {
        "SDL_JOYSTICK_HIDAPI_PS3": "1",
        "SDL_JOYSTICK_HIDAPI_WII": "1",
        "SDL_JOYSTICK_HIDAPI_XBOX": "0" if sources.get("SDLJoystickXboxHIDAPI") == "false" else "1",
        "SDL_JOYSTICK_LINUX_DIGITAL_HATS": "1" if sources.get("SDLJoystickLinuxDigitalHats") == "true" else "0",
        "SDL_JOYSTICK_ENHANCED_REPORTS": "auto" if sources.get("SDLControllerEnhancedMode") == "true" else "0",
    }
    return {**out, **extra_hints(text)}


def plan(ctx: Context):
    return write_pads(config_path(ctx.runner_path), slots, ctx, name="DuckStation", face=FACE, taken=TAKEN, pad_type="AnalogController")
