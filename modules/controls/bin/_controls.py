import os
import re
from dataclasses import dataclass
from pathlib import Path

from _gamepads import BIND_AXIS, BIND_BUTTON, Pad

SOUTH, EAST, WEST, NORTH = 0, 1, 2, 3
BACK, GUIDE, START, LEFT_STICK, RIGHT_STICK, LEFT_SHOULDER, RIGHT_SHOULDER = 4, 5, 6, 7, 8, 9, 10
DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT = 11, 12, 13, 14
LEFT_X, LEFT_Y, RIGHT_X, RIGHT_Y, TRIGGER_LEFT, TRIGGER_RIGHT = 0, 1, 2, 3, 4, 5

# A Nintendo pad's A/B/X/Y on the held pad's face buttons.
FACE = {
    "positional": {"a": EAST, "b": SOUTH, "x": NORTH, "y": WEST},
    "xbox": {"a": SOUTH, "b": EAST, "x": WEST, "y": NORTH},
}
WIIMOTE = ("nunchuk", "sideways", "classic")
SHOULDERS = ("standard", "swapped")

# SDL3 gamepad outputs Universe keeps: guide, misc1, the four paddles, misc2-6.
TAKEN = (GUIDE, 15, 16, 17, 18, 19, 21, 22, 23, 24, 25)


class Skip(Exception): ...


@dataclass
class Context:
    pads: list[Pad]
    layout: str = "positional"
    guide: bool = False
    runner_path: str = ""
    platform: str = ""
    wiimote: str = "nunchuk"
    shoulders: str = "standard"

    @property
    def face(self):
        return FACE[self.layout]


def config_home():
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")


def data_home():
    return Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")


def ordinals(pads, key):
    """How many pads sharing a pad's `key` SDL lists before it: emulators number twins in SDL's order, not the players'."""
    return [sum(1 for q in pads if key(q) == key(p) and q.index < p.index) for p in pads]


def first_file(*candidates):
    return next((p for p in candidates if p.is_file()), None)


def button(pad, b):
    return pad.bindings.get((BIND_BUTTON, b))


def axis(pad, a):
    return pad.bindings.get((BIND_AXIS, a))


def first_pad(ctx: Context):
    if not ctx.pads:
        raise Skip("no pad held: controls left as they are")
    return ctx.pads[0]


def shoulders(ctx: Context, lb, rb, lt, rt):
    return (lt, rt, lb, rb) if ctx.shoulders == "swapped" else (lb, rb, lt, rt)


def held_shoulders(pad):
    return button(pad, LEFT_SHOULDER), button(pad, RIGHT_SHOULDER), axis(pad, TRIGGER_LEFT), axis(pad, TRIGGER_RIGHT)


def pad_shoulders(ctx: Context, pad):
    return shoulders(ctx, *held_shoulders(pad))


def swap_names(ctx: Context, values, lb, rb, lt, rt):
    swap = dict(zip((lb, rb, lt, rt), shoulders(ctx, lb, rb, lt, rt), strict=True))
    pattern = re.compile(rf"(?<![A-Za-z])({'|'.join(map(re.escape, swap))})(?![A-Za-z])")
    return {k: pattern.sub(lambda m: swap[m.group()], v) if isinstance(v, str) else v for k, v in values.items()}


def taken_buttons(pad, guide=False):
    return {b.index for out in TAKEN if not (guide and out == GUIDE) and (b := button(pad, out)) and b.kind == BIND_BUTTON}


def unquote(value):
    return value[1:-1] if len(value) >= 2 and value[0] == value[-1] == '"' else value


def header(line):
    s = line.strip().lstrip("\ufeff")
    return s[1:-1] if s.startswith("[") and s.endswith("]") else None


def key_of(line):
    s = line.strip()
    if not s or s[0] in "#;" or "=" not in s:
        return None
    return s.split("=", 1)[0].strip()


def value_of(line):
    return line.split("=", 1)[1].strip()


def _bounds(lines, name):
    start = next((i for i, line in enumerate(lines) if header(line) == name), None)
    if start is None:
        return None
    end = next((i for i in range(start + 1, len(lines)) if header(lines[i]) is not None), len(lines))
    while end > start + 1 and not lines[end - 1].strip():
        end -= 1
    return start, end


def sections(text):
    return [h for h in map(header, text.splitlines()) if h is not None]


def section_values(text, name):
    lines = text.splitlines()
    bounds = _bounds(lines, name)
    body = lines[bounds[0] + 1 : bounds[1]] if bounds else []
    return {k: value_of(line) for line in body if (k := key_of(line))}


def ini_section(text, name):
    return {k: unquote(v) for k, v in section_values(text, name).items()}


def edit_section(text, name, update):
    """`update` gets and returns the section's line list: a key repeats once per binding in some files. A missing section is added only when `update` gives it lines."""
    lines = text.splitlines()
    bounds = _bounds(lines, name)
    if bounds is None:
        body = update([])
        if not body:
            return text
        out = [*lines, *([""] if lines and lines[-1].strip() else []), f"[{name}]", *body]
    else:
        start, end = bounds
        out = [*lines[: start + 1], *update(lines[start + 1 : end]), *lines[end:]]
    return "\n".join(out) + "\n"


def ini_rewrite(text, name, lines, drop=()):
    """`lines` maps a key to its whole line, set in place or added at the section's end; keys in `drop` go."""

    def update(body):
        kept = [line for line in body if key_of(line) not in drop]
        keys = {key_of(line) for line in kept}
        return [lines.get(key_of(line), line) for line in kept] + [v for k, v in lines.items() if k not in keys]

    return edit_section(text, name, update)


def ini_set(text, name, values, sep="="):
    return ini_rewrite(text, name, {k: f"{k}{sep}{v}" for k, v in values.items()})
