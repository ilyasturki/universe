from _controls import (
    BACK,
    DPAD_DOWN,
    DPAD_LEFT,
    DPAD_RIGHT,
    DPAD_UP,
    LEFT_SHOULDER,
    LEFT_X,
    LEFT_Y,
    RIGHT_SHOULDER,
    START,
    Context,
    Skip,
    axis,
    button,
    config_home,
    first_file,
    ini_section,
    ini_set,
    taken_buttons,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

INSTANCE = "Instance0"
JOYSTICK = "Instance0.Joystick"
UNSET = -1
# EmuInstanceInput's packing: low 16 bits a button or a hat (0x100 | hat<<4 | direction), 0xFFFF for none; 0x10000 flags an axis part, its mode at bit 20, its number at bit 24.
NO_BUTTON = 0xFFFF
HAT_FLAG = 0x100
AXIS_FLAG = 0x10000
ABOVE_HALF, BELOW_HALF, TRIGGER = 0, 1, 2
BUTTONS = {"L": LEFT_SHOULDER, "R": RIGHT_SHOULDER, "Select": BACK, "Start": START}
# The d-pad, each direction also taken from the left stick: (d-pad button, stick axis, the stick's half).
DPAD = {"Up": (DPAD_UP, LEFT_Y, -1), "Down": (DPAD_DOWN, LEFT_Y, 1), "Left": (DPAD_LEFT, LEFT_X, -1), "Right": (DPAD_RIGHT, LEFT_X, 1)}


def _button_part(binding):
    if binding is None:
        return None
    if binding.kind == BIND_BUTTON and binding.index <= 0xFE:
        return binding.index
    if binding.kind == BIND_HAT and binding.index <= 0xF and binding.mask in (1, 2, 4, 8):
        return HAT_FLAG | binding.index << 4 | binding.mask
    return None


def _axis_part(index, mode):
    return AXIS_FLAG | mode << 20 | index << 24 if index <= 0xF else None


def encode(binding):
    part = _button_part(binding)
    if part is not None:
        return part
    if binding is not None and binding.kind == BIND_AXIS:
        full = binding.lo <= -32768 and binding.hi >= 32767
        mode = TRIGGER if full else ABOVE_HALF if binding.hi > binding.lo else BELOW_HALF
        a = _axis_part(binding.index, mode)
        if a is not None:
            return NO_BUTTON | a
    return UNSET


def _dpad(pad, dpad_button, stick, half):
    part = _button_part(button(pad, dpad_button))
    b = axis(pad, stick)
    a = None
    if b is not None and b.kind == BIND_AXIS:
        positive = (half > 0) == (b.hi >= b.lo)
        a = _axis_part(b.index, ABOVE_HALF if positive else BELOW_HALF)
    if part is None and a is None:
        return UNSET
    return (NO_BUTTON if part is None else part) | (a or 0)


def values_for(ctx: Context):
    pad = ctx.pads[0]
    values = {k.upper(): encode(button(pad, b)) for k, b in ctx.face.items()}
    values.update({k: encode(button(pad, b)) for k, b in BUTTONS.items()})
    values.update({k: _dpad(pad, *spec) for k, spec in DPAD.items()})
    return values


def cleared(value, taken):
    low = value & 0xFFFF
    if value < 0 or low == NO_BUTTON or low & HAT_FLAG or low not in taken:
        return value
    return value & ~0xFFFF | NO_BUTTON if value & AXIS_FLAG else UNSET


def rewrite(text, ctx: Context):
    values = values_for(ctx)
    taken = taken_buttons(ctx.pads[0], ctx.guide)
    for key, raw in ini_section(text, JOYSTICK).items():
        if key.startswith("HK_") and key not in values:
            try:
                value = int(raw)
            except ValueError:
                continue
            if cleared(value, taken) != value:
                values[key] = cleared(value, taken)
    text = ini_set(text, INSTANCE, {"JoystickID": ctx.pads[0].index}, sep=" = ")
    return ini_set(text, JOYSTICK, values, sep=" = ")


def plan(ctx: Context):
    path = first_file(config_home() / "melonDS" / "melonDS.toml")
    if path is None:
        raise Skip("no melonDS/melonDS.toml: start melonDS once, then its controls are written")
    return {path: rewrite(path.read_text(), ctx)}
