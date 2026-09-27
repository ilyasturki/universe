import os

from _controls import (
    BACK,
    DPAD_DOWN,
    DPAD_LEFT,
    DPAD_RIGHT,
    DPAD_UP,
    LEFT_X,
    LEFT_Y,
    START,
    Context,
    Skip,
    axis,
    button,
    config_home,
    ini_section,
    ini_set,
    pad_shoulders,
    taken_buttons,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

# Joypad1-5 with a multitap on port 2; [Joypad K] and [Joypad K+5] are two binding sets of SNES pad K+1.
PLAYERS = 5
SETS = 5
UNSET = "Unset"
TURBO_STICKY = [f"{kind} {b}" for kind in ("Turbo", "Sticky") for b in ("A", "B", "X", "Y", "L", "R")]
DPAD = {"Up": DPAD_UP, "Down": DPAD_DOWN, "Left": DPAD_LEFT, "Right": DPAD_RIGHT}
# A hat is two pseudo-axes after the real ones: vertical at nax+2h (up positive), horizontal at nax+2h+1.
HAT = {1: (0, "+"), 4: (0, "-"), 8: (1, "-"), 2: (1, "+")}


def _source(pad, binding, joystick):
    if binding is None:
        return UNSET
    if binding.kind == BIND_BUTTON:
        return f"{joystick} Button {binding.index}"
    if binding.kind == BIND_HAT and binding.mask in HAT:
        offset, sign = HAT[binding.mask]
        return f"{joystick} Axis {pad.axes + 2 * binding.index + offset} {sign} 50%"
    if binding.kind == BIND_AXIS:
        sign = 1 if binding.hi > binding.lo else -1
        # A threshold is a share of the travel from 0 (snes9x calibrates only on request): a trigger resting at -32768 presses past 0%.
        middle = sign * (binding.lo + binding.hi) / 2
        return f"{joystick} Axis {binding.index} {'+' if sign > 0 else '-'} {max(0, round(middle * 100 / 32767))}%"
    return UNSET


def pad_sets(pad, ctx: Context):
    joystick = f"Joystick {pad.index + 1}"
    buttons = {**{k.upper(): b for k, b in ctx.face.items()}, "Select": BACK, "Start": START, **DPAD}
    lb, rb, lt, rt = pad_shoulders(ctx, pad)
    main = {
        **{k: _source(pad, button(pad, b), joystick) for k, b in buttons.items()},
        "L": _source(pad, lb, joystick),
        "R": _source(pad, rb, joystick),
        **dict.fromkeys(TURBO_STICKY, UNSET),
    }
    stick = dict.fromkeys(main, UNSET)
    stick.update({"L": _source(pad, lt, joystick), "R": _source(pad, rt, joystick)})
    for out, names in ((LEFT_X, ("Left", "Right")), (LEFT_Y, ("Up", "Down"))):
        a = axis(pad, out)
        if a and a.kind == BIND_AXIS:
            neg, pos = names[::-1] if a.lo > a.hi else names
            stick[neg] = f"{joystick} Axis {a.index} - 50%"
            stick[pos] = f"{joystick} Axis {a.index} + 50%"
    return main, stick


def _joystick_of(value):
    parts = value.split()
    if len(parts) == 4 and parts[0] == "Joystick" and parts[2] == "Button" and parts[1].isdigit() and parts[3].isdigit():
        return int(parts[1]) - 1, int(parts[3])
    return None


def plan(ctx: Context):
    if "gtk" not in os.path.basename(ctx.runner_path):
        raise Skip(f"{os.path.basename(ctx.runner_path) or 'snes9x'} is snes9x's X11 build, which reads /dev/input/js* itself: controls left as they are")
    path = config_home() / "snes9x" / "snes9x.conf"
    text = path.read_text() if path.is_file() else ""
    pads = ctx.pads[:PLAYERS]
    text = ini_set(text, "Input", {"ControllerPort0": "joypad", "ControllerPort1": "multitap" if len(pads) >= 3 else "joypad"}, sep=" = ")
    for k, pad in enumerate(pads):
        main, stick = pad_sets(pad, ctx)
        text = ini_set(text, f"Joypad {k}", main, sep=" = ")
        text = ini_set(text, f"Joypad {k + SETS}", stick, sep=" = ")
    for k in range(len(pads), PLAYERS):
        for section in (f"Joypad {k}", f"Joypad {k + SETS}"):
            stale = {key: UNSET for key, v in ini_section(text, section).items() if v.startswith("Joystick ")}
            if stale:
                text = ini_set(text, section, stale, sep=" = ")
    taken = {(p.index, b) for p in pads for b in taken_buttons(p)}
    shortcuts = {key: UNSET for key, v in ini_section(text, "Shortcuts").items() if _joystick_of(v) in taken}
    if shortcuts:
        text = ini_set(text, "Shortcuts", shortcuts, sep=" = ")
    return {path: text}
