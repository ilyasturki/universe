import os
from pathlib import Path

from _sdl import BIND_AXIS, BIND_BUTTON, BIND_HAT, Pad

# The core's order for the forks sharing Eden's config layout (roms.rs EDEN_CONFIGS).
CONFIG_DIRS = ("eden", "citron", "sudachi", "suyu", "yuzu")
SECTION = "Controls"
# player_8 is the handheld console, player_9 the debug pad.
PLAYERS = 8
EMPTY = "[empty]"

SOUTH, EAST, WEST, NORTH = 0, 1, 2, 3
BACK, GUIDE, START, LEFT_STICK, RIGHT_STICK, LEFT_SHOULDER, RIGHT_SHOULDER = 4, 5, 6, 7, 8, 9, 10
DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT = 11, 12, 13, 14
LEFT_X, LEFT_Y, RIGHT_X, RIGHT_Y, TRIGGER_LEFT, TRIGGER_RIGHT = 0, 1, 2, 3, 4, 5

FACE = {
    "positional": {"a": EAST, "b": SOUTH, "x": NORTH, "y": WEST},
    "xbox": {"a": SOUTH, "b": EAST, "x": WEST, "y": NORTH},
}
# Eden's GetDefaultButtonBinding past the face buttons; SL/SR land on the shoulders on a pad that is no Joy-Con.
BUTTONS = {
    "lstick": LEFT_STICK,
    "rstick": RIGHT_STICK,
    "l": LEFT_SHOULDER,
    "r": RIGHT_SHOULDER,
    "plus": START,
    "minus": BACK,
    "dleft": DPAD_LEFT,
    "dup": DPAD_UP,
    "dright": DPAD_RIGHT,
    "ddown": DPAD_DOWN,
    "slleft": LEFT_SHOULDER,
    "srleft": RIGHT_SHOULDER,
    "slright": LEFT_SHOULDER,
    "srright": RIGHT_SHOULDER,
}
HAT = {1: "up", 2: "right", 4: "down", 8: "left"}


def config_path(config_home=None):
    root = Path(config_home or os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return next((p for p in (root / d / "qt-config.ini" for d in CONFIG_DIRS) if p.is_file()), None)


def _unquote(value):
    return value[1:-1] if len(value) >= 2 and value[0] == value[-1] == '"' else value


def section(text, name=SECTION):
    out, current = {}, None
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            current = s[1:-1]
        elif current == name and "=" in s:
            k, v = s.split("=", 1)
            out[k.strip()] = _unquote(v.strip())
    return out


def _flag(controls, key, default):
    return controls[key] == "true" if key in controls else default


def driver_hints(controls):
    """Eden's SDL driver hints: they pick the driver that claims each pad, and with it the GUID and indices."""
    return {
        "SDL_JOYSTICK_HIDAPI_XBOX": "0",
        "SDL_JOYSTICK_HIDAPI_JOY_CONS": "0" if _flag(controls, "enable_joycon_driver", True) else "1",
        "SDL_JOYSTICK_HIDAPI_SWITCH": "0" if _flag(controls, "enable_procon_driver", False) else "1",
    }


def eden_guid(guid):
    """Eden keys a pad by its SDL GUID with the name CRC (bytes 2-3) zeroed."""
    return (guid[:2] + b"\0\0" + guid[4:]).hex()


def ports(pads):
    """Eden numbers pads sharing a GUID in the order SDL adds them."""
    seen, out = {}, []
    for p in pads:
        g = eden_guid(p.guid)
        out.append(seen.get(g, 0))
        seen[g] = out[-1] + 1
    return out


def _param(head, binding):
    if binding is None:
        return EMPTY
    kind, index, mask = binding
    if kind == BIND_BUTTON:
        return f"{head},button:{index}"
    if kind == BIND_HAT and mask in HAT:
        return f"{head},hat:{index},direction:{HAT[mask]}"
    if kind == BIND_AXIS:
        return f"{head},axis:{index},threshold:0.5,invert:+"
    return EMPTY


def _stick(head, pad, x, y):
    bx, by = pad.bindings.get((BIND_AXIS, x)), pad.bindings.get((BIND_AXIS, y))
    if not bx or not by or bx[0] != BIND_AXIS or by[0] != BIND_AXIS:
        return EMPTY
    return f"{head},axis_x:{bx[1]},axis_y:{by[1]},offset_x:0.000000,offset_y:0.000000,invert_x:+,invert_y:+"


def player_values(player, pad, port, layout, guide):
    head = f"engine:sdl,port:{port},guid:{eden_guid(pad.guid)}"
    buttons = {**FACE[layout], **BUTTONS}
    values = {f"button_{k}": _param(head, pad.bindings.get((BIND_BUTTON, b))) for k, b in buttons.items()}
    values["button_zl"] = _param(head, pad.bindings.get((BIND_AXIS, TRIGGER_LEFT)))
    values["button_zr"] = _param(head, pad.bindings.get((BIND_AXIS, TRIGGER_RIGHT)))
    values["button_home"] = _param(head, pad.bindings.get((BIND_BUTTON, GUIDE))) if guide else EMPTY
    values["button_screenshot"] = EMPTY
    values["lstick"] = _stick(head, pad, LEFT_X, LEFT_Y)
    values["rstick"] = _stick(head, pad, RIGHT_X, RIGHT_Y)
    motion = f"engine:sdl,motion:0,port:{port},guid:{eden_guid(pad.guid)}" if pad.gyro else EMPTY
    values["motionleft"] = values["motionright"] = motion
    out = {f"player_{player}_{k}": v for k, v in values.items()}
    out[f"player_{player}_type"] = "0"
    out[f"player_{player}_connected"] = "true"
    return out


def values_for(pads: list[Pad], layout, guide):
    out = {}
    for player, (pad, port) in enumerate(zip(pads[:PLAYERS], ports(pads[:PLAYERS]), strict=True)):
        out.update(player_values(player, pad, port, layout, guide))
    for player in range(len(pads[:PLAYERS]), PLAYERS):
        out[f"player_{player}_connected"] = "false"
    return out


def _encode(value):
    return value if value in (EMPTY, "true", "false") or value.isdigit() else f'"{value}"'


def rewrite(text, values, name=SECTION):
    """A value Eden reads needs its `\\default=false` beside it, else Eden takes its built-in default."""
    wanted = {}
    for k, v in values.items():
        wanted[f"{k}\\default"] = f"{k}\\default=false"
        wanted[k] = f"{k}={_encode(v)}"
    out, current, done, end = [], None, set(), None
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            if current == name:
                end = len(out)
            current = s[1:-1]
        elif current == name and "=" in s:
            key = s.split("=", 1)[0].strip()
            if key in wanted:
                out.append(wanted[key])
                done.add(key)
                continue
        out.append(line)
    if current == name:
        end = len(out)
    missing = [line for k, line in wanted.items() if k not in done]
    if end is None:
        out += ["", f"[{name}]", *missing]
    else:
        while end > 0 and not out[end - 1].strip():
            end -= 1
        out[end:end] = missing
    return "\n".join(out) + "\n"
