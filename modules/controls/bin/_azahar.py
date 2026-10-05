from _controls import (
    BACK,
    DPAD_DOWN,
    DPAD_LEFT,
    DPAD_RIGHT,
    DPAD_UP,
    GUIDE,
    LEFT_X,
    LEFT_Y,
    RIGHT_X,
    RIGHT_Y,
    START,
    Context,
    Skip,
    axis,
    button,
    config_home,
    first_file,
    first_pad,
    ini_rewrite,
    ini_section,
    ordinals,
    pad_shoulders,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

SECTION = "Controls"
EMPTY = "[empty]"
HAT = {1: "up", 2: "right", 4: "down", 8: "left"}
BUTTONS = {
    "up": DPAD_UP,
    "down": DPAD_DOWN,
    "left": DPAD_LEFT,
    "right": DPAD_RIGHT,
    "start": START,
    "select": BACK,
}
MOTION_EMU = {"engine": "motion_emu", "sensitivity": "0.01", "tilt_clamp": "90.0", "update_period": "100"}


def _escape(value):
    return str(value).replace("$", "$2").replace(":", "$0").replace(",", "$1")


def package(params):
    """Azahar's ParamPackage text: keys sorted as its std::map keeps them."""
    return ",".join(f"{k}:{_escape(params[k])}" for k in sorted(params)) if params else EMPTY


def _threshold(binding):
    """Azahar's own automap presses past the middle of the input's range: a full-range trigger at 0, a half axis at 0.5."""
    middle = round((binding.lo + binding.hi) / 2 / 32767, 2)
    return f"{middle + 0.0:.6f}"


def _param(device, binding):
    if binding is None:
        return EMPTY
    if binding.kind == BIND_BUTTON:
        return package({**device, "button": binding.index})
    if binding.kind == BIND_HAT and binding.mask in HAT:
        return package({**device, "hat": binding.index, "direction": HAT[binding.mask]})
    if binding.kind == BIND_AXIS:
        direction = "+" if binding.hi >= binding.lo else "-"
        return package({**device, "axis": binding.index, "direction": direction, "threshold": _threshold(binding)})
    return EMPTY


def _stick(device, pad, x, y):
    bx, by = axis(pad, x), axis(pad, y)
    if not bx or not by or bx.kind != BIND_AXIS or by.kind != BIND_AXIS:
        return EMPTY
    return package({**device, "axis_x": bx.index, "axis_y": by.index, "deadzone": "0.100000"})


def values_for(ctx: Context):
    pad = first_pad(ctx)
    device = {"engine": "sdl", "guid": pad.guid.hex(), "port": ordinals(ctx.pads, lambda p: p.guid)[0]}
    values = {f"button_{k}": _param(device, button(pad, b)) for k, b in {**ctx.face, **BUTTONS}.items()}
    held = dict(zip(("button_l", "button_r", "button_zl", "button_zr"), pad_shoulders(ctx, pad), strict=True))
    values.update({k: _param(device, b) for k, b in held.items()})
    values["button_home"] = _param(device, button(pad, GUIDE)) if ctx.guide else EMPTY
    values["circle_pad"] = _stick(device, pad, LEFT_X, LEFT_Y)
    values["c_stick"] = _stick(device, pad, RIGHT_X, RIGHT_Y)
    values["motion_device"] = package(device if pad.gyro else MOTION_EMU)
    return values


def _encode(value):
    return value if value == EMPTY else f'"{value}"'


def rewrite(text, values):
    """Azahar ignores a value without its `\\default=false`."""
    controls = ini_section(text, SECTION)
    try:
        profile = int(controls.get("profile", "0"))
    except ValueError:
        profile = 0
    prefix = f"profiles\\{profile + 1}\\"
    lines = {}
    for k, v in values.items():
        lines[f"{prefix}{k}\\default"] = f"{prefix}{k}\\default=false"
        lines[f"{prefix}{k}"] = f"{prefix}{k}={_encode(v)}"
    try:
        size = int(controls.get("profiles\\size", "0"))
    except ValueError:
        size = 0
    if size < profile + 1:
        lines["profiles\\size"] = f"profiles\\size={profile + 1}"
    return ini_rewrite(text, SECTION, lines)


def plan(ctx: Context):
    path = first_file(config_home() / "azahar-emu" / "qt-config.ini")
    if path is None:
        raise Skip("no azahar-emu/qt-config.ini: start Azahar once, then its controls are written")
    return {path: rewrite(path.read_text(), values_for(ctx))}
