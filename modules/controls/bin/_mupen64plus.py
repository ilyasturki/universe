import os

from _controls import (
    DPAD_DOWN,
    DPAD_LEFT,
    DPAD_RIGHT,
    DPAD_UP,
    LEFT_X,
    LEFT_Y,
    RIGHT_X,
    RIGHT_Y,
    SOUTH,
    START,
    WEST,
    Context,
    Skip,
    axis,
    button,
    config_home,
    edit_section,
    pad_shoulders,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

PLAYERS = 4
HAT = {1: "Up", 2: "Right", 4: "Down", 8: "Left"}
# input-sdl resets a section whose (int)version is not 2 to full auto.
HEADER = {"version": "2.000000", "mode": "0"}
MEM_PAK = 2


def _source(binding):
    if binding is None:
        return ""
    if binding.kind == BIND_BUTTON:
        return f"button({binding.index})"
    if binding.kind == BIND_HAT and binding.mask in HAT:
        return f"hat({binding.index} {HAT[binding.mask]})"
    if binding.kind == BIND_AXIS:
        return f"axis({binding.index}{'+' if binding.hi > binding.lo else '-'})"
    return ""


def _stick(pad, out):
    a = axis(pad, out)
    if not a or a.kind != BIND_AXIS:
        return None, None
    neg, pos = f"{a.index}-", f"{a.index}+"
    return (pos, neg) if a.lo > a.hi else (neg, pos)


def _quote(value):
    return '"' + value.replace('"', "") + '"'


def player(pad, ctx: Context):
    """mupen's own convention for a modern pad: A on SOUTH, B on WEST, C on the right stick, Z on the left trigger."""
    rx_neg, rx_pos = _stick(pad, RIGHT_X)
    ry_neg, ry_pos = _stick(pad, RIGHT_Y)
    lx_neg, lx_pos = _stick(pad, LEFT_X)
    ly_neg, ly_pos = _stick(pad, LEFT_Y)
    lb, rb, lt, rt = pad_shoulders(ctx, pad)
    # A value holds one token of each kind, so a digital right trigger cannot join the shoulder's button().
    r = _source(rb)
    if rt and (not rb or rt.kind != rb.kind):
        r = f"{r} {_source(rt)}".strip()
    values = {
        "DPad R": _source(button(pad, DPAD_RIGHT)),
        "DPad L": _source(button(pad, DPAD_LEFT)),
        "DPad D": _source(button(pad, DPAD_DOWN)),
        "DPad U": _source(button(pad, DPAD_UP)),
        "Start": _source(button(pad, START)),
        "Z Trig": _source(lt),
        "B Button": _source(button(pad, WEST)),
        "A Button": _source(button(pad, SOUTH)),
        "C Button R": f"axis({rx_pos})" if rx_pos else "",
        "C Button L": f"axis({rx_neg})" if rx_neg else "",
        "C Button D": f"axis({ry_pos})" if ry_pos else "",
        "C Button U": f"axis({ry_neg})" if ry_neg else "",
        "R Trig": r,
        "L Trig": _source(lb),
        "Mempak switch": "",
        "Rumblepak switch": "",
        "X Axis": f"axis({lx_neg},{lx_pos})" if lx_neg else "",
        "Y Axis": f"axis({ly_neg},{ly_pos})" if ly_neg else "",
    }
    return {
        **HEADER,
        "device": str(pad.index),
        "name": _quote(pad.joystick_name or pad.name),
        "plugged": "True",
        "plugin": str(MEM_PAK),
        "mouse": "False",
        "MouseSensitivity": '"2.00,2.00"',
        "AnalogDeadzone": '"4096,4096"',
        "AnalogPeak": '"32768,32768"',
        **{k: _quote(v) for k, v in values.items()},
    }


UNPLUGGED = {**HEADER, "device": "-1", "name": '""', "plugged": "False"}


def plan(ctx: Context):
    if os.path.basename(ctx.runner_path).startswith("m64p"):
        raise Skip("m64p keeps its own input settings: controls left as they are")
    path = config_home() / "mupen64plus" / "mupen64plus.cfg"
    text = path.read_text() if path.is_file() else ""
    pads = ctx.pads[:PLAYERS]
    for n in range(PLAYERS):
        values = player(pads[n], ctx) if n < len(pads) else UNPLUGGED
        text = edit_section(text, f"Input-SDL-Control{n + 1}", lambda _, v=values: [f"{k} = {x}" for k, x in v.items()])
    return {path: text}
