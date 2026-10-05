from _controls import (
    BACK,
    DPAD_DOWN,
    DPAD_LEFT,
    DPAD_RIGHT,
    DPAD_UP,
    GUIDE,
    LEFT_STICK,
    LEFT_X,
    LEFT_Y,
    RIGHT_STICK,
    RIGHT_X,
    RIGHT_Y,
    START,
    Context,
    Skip,
    axis,
    button,
    config_home,
    first_file,
    ini_rewrite,
    ini_section,
    ordinals,
    pad_shoulders,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

# The core's order for the forks sharing Eden's config layout (roms.rs EDEN_CONFIGS).
CONFIG_DIRS = ("eden", "citron", "sudachi", "suyu", "yuzu")
SECTION = "Controls"
# player_8 is the handheld console, player_9 the debug pad.
PLAYERS = 8
EMPTY = "[empty]"

# Eden's GetDefaultButtonBinding past the face buttons; SL/SR land on the shoulders on a pad that is no Joy-Con.
BUTTONS = {
    "lstick": LEFT_STICK,
    "rstick": RIGHT_STICK,
    "plus": START,
    "minus": BACK,
    "dleft": DPAD_LEFT,
    "dup": DPAD_UP,
    "dright": DPAD_RIGHT,
    "ddown": DPAD_DOWN,
}
# Each onto the pad's LB, RB, LT or RT.
SHOULDERS = {"l": 0, "r": 1, "zl": 2, "zr": 3, "slleft": 0, "srleft": 1, "slright": 0, "srright": 1}
HAT = {1: "up", 2: "right", 4: "down", 8: "left"}


def config_path():
    return first_file(*(config_home() / d / "qt-config.ini" for d in CONFIG_DIRS))


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
    return ordinals(pads, lambda p: eden_guid(p.guid))


def _param(head, binding):
    if binding is None:
        return EMPTY
    if binding.kind == BIND_BUTTON:
        return f"{head},button:{binding.index}"
    if binding.kind == BIND_HAT and binding.mask in HAT:
        return f"{head},hat:{binding.index},direction:{HAT[binding.mask]}"
    if binding.kind == BIND_AXIS:
        return f"{head},axis:{binding.index},threshold:0.5,invert:+"
    return EMPTY


def _stick(head, pad, x, y):
    bx, by = axis(pad, x), axis(pad, y)
    if not bx or not by or bx.kind != BIND_AXIS or by.kind != BIND_AXIS:
        return EMPTY
    return f"{head},axis_x:{bx.index},axis_y:{by.index},offset_x:0.000000,offset_y:0.000000,invert_x:+,invert_y:+"


def player_values(player, pad, port, ctx: Context):
    head = f"engine:sdl,port:{port},guid:{eden_guid(pad.guid)}"
    values = {f"button_{k}": _param(head, button(pad, b)) for k, b in {**ctx.face, **BUTTONS}.items()}
    held = pad_shoulders(ctx, pad)
    values.update({f"button_{k}": _param(head, held[i]) for k, i in SHOULDERS.items()})
    values["button_home"] = _param(head, button(pad, GUIDE)) if ctx.guide else EMPTY
    values["button_screenshot"] = EMPTY
    values["lstick"] = _stick(head, pad, LEFT_X, LEFT_Y)
    values["rstick"] = _stick(head, pad, RIGHT_X, RIGHT_Y)
    motion = f"engine:sdl,motion:0,port:{port},guid:{eden_guid(pad.guid)}" if pad.gyro else EMPTY
    values["motionleft"] = values["motionright"] = motion
    out = {f"player_{player}_{k}": v for k, v in values.items()}
    out[f"player_{player}_type"] = "0"
    out[f"player_{player}_connected"] = "true"
    return out


def values_for(ctx: Context):
    pads = ctx.pads[:PLAYERS]
    out = {}
    for player, (pad, port) in enumerate(zip(pads, ports(pads), strict=True)):
        out.update(player_values(player, pad, port, ctx))
    for player in range(len(pads), PLAYERS):
        out[f"player_{player}_connected"] = "false"
    return out


def _encode(value):
    return value if value in (EMPTY, "true", "false") or value.isdigit() else f'"{value}"'


def rewrite(text, values):
    """A value Eden reads needs its `\\default=false` beside it, else Eden takes its built-in default."""
    lines = {}
    for k, v in values.items():
        lines[f"{k}\\default"] = f"{k}\\default=false"
        lines[k] = f"{k}={_encode(v)}"
    return ini_rewrite(text, SECTION, lines)


def hints():
    path = config_path()
    return driver_hints(ini_section(path.read_text(), SECTION) if path else {})


def plan(ctx: Context):
    path = config_path()
    if path is None:
        raise Skip("no qt-config.ini under eden, citron, sudachi, suyu or yuzu: start Eden once, then its controls are written")
    return {path: rewrite(path.read_text(), values_for(ctx))}
