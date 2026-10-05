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
    first_pad,
    held_shoulders,
    ini_rewrite,
    ini_section,
    taken_buttons,
)
from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT

SDLB = "gba.input.SDLB"
# mGBA reads at most four players' device<N>.
PLAYERS = 4
THRESHOLD = 16384
# GBA key indices, as hat<h><Dir> values name them.
KEY_INDEX = {"A": 0, "B": 1, "Select": 2, "Start": 3, "Right": 4, "Left": 5, "Up": 6, "Down": 7, "R": 8, "L": 9}
DPAD = {"Up": DPAD_UP, "Down": DPAD_DOWN, "Left": DPAD_LEFT, "Right": DPAD_RIGHT}
HAT_DIRS = {1: "Up", 2: "Right", 4: "Down", 8: "Left"}
BINDING_PREFIXES = ("key", "axis", "hat")


def config_dir():
    return config_home() / "mgba"


def _raw(binding):
    return binding.index if binding and binding.kind == BIND_BUTTON else -1


def bindings(pad, ctx: Context):
    keys = {"A": ctx.face["a"], "B": ctx.face["b"], "Select": BACK, "Start": START}
    out = {f"key{k}": _raw(button(pad, b)) for k, b in keys.items()}
    lb, rb, lt, rt = held_shoulders(pad)
    for key, inputs in (("L", (lb, lt)), ("R", (rb, rt))):
        out[f"key{key}"] = next((b.index for b in inputs if b and b.kind == BIND_BUTTON), -1)
        a = next((b for b in inputs if b and b.kind == BIND_AXIS), None)
        if a:
            # mGBA presses past axis<Key>Value: the middle of the trigger's travel, on the side it moves to.
            out[f"axis{key}Axis"] = f"{'+' if a.hi > a.lo else '-'}{a.index}"
            out[f"axis{key}Value"] = (a.lo + a.hi) // 2
    hats = {}
    for name, out_button in DPAD.items():
        b = button(pad, out_button)
        out[f"key{name}"] = _raw(b)
        if b and b.kind == BIND_HAT and b.mask in HAT_DIRS:
            hats.setdefault(b.index, {})[HAT_DIRS[b.mask]] = KEY_INDEX[name]
    # mGBA stops loading hats at the first id with no key, so the ones below a used hat are written unbound.
    for h in range(max(hats) + 1 if hats else 0):
        for d in ("Up", "Right", "Down", "Left"):
            out[f"hat{h}{d}"] = hats.get(h, {}).get(d, -1)
    for stick, names in ((LEFT_X, ("Left", "Right")), (LEFT_Y, ("Up", "Down"))):
        a = axis(pad, stick)
        if not a or a.kind != BIND_AXIS:
            continue
        neg, pos = names[::-1] if a.lo > a.hi else names
        out[f"axis{neg}Axis"] = f"-{a.index}"
        out[f"axis{neg}Value"] = -THRESHOLD
        out[f"axis{pos}Axis"] = f"+{a.index}"
        out[f"axis{pos}Value"] = THRESHOLD
    return out


def _replace_bindings(text, name, values, extra=None):
    """Layers overlay key by key, so a section's old key*/axis*/hat* go before the new set lands."""
    old = [k for k in ini_section(text, name) if k.startswith(BINDING_PREFIXES)]
    lines = {**{k: f"{k}={v}" for k, v in (extra or {}).items()}, **{k: f"{k}={v}" for k, v in values.items()}}
    return ini_rewrite(text, name, lines, drop=[k for k in old if k not in (extra or {})])


def qt_escape(name):
    out = []
    for ch in name:
        c = ord(ch)
        if ch == "/":
            out.append("\\")
        elif ch.isascii() and (ch.isalnum() or ch in "_-."):
            out.append(ch)
        elif c <= 0xFF:
            out.append(f"%{c:02X}")
        else:
            out.append(f"%U{c:04X}")
    return "".join(out)


def _free_shortcuts(text, pads):
    for pad in pads:
        taken = {str(b) for b in taken_buttons(pad)}
        name = f"shortcutProfileButton.{qt_escape(pad.joystick_name)}"
        stale = {k: f"{k}=-1" for k, v in ini_section(text, name).items() if v in taken}
        if stale:
            text = ini_rewrite(text, name, stale)
    return text


def plan(ctx: Context):
    path = config_dir() / "config.ini"
    if not path.is_file():
        raise Skip("no mgba/config.ini: start mGBA once, then its controls are written")
    lead = first_pad(ctx)
    text = path.read_text()
    pads = ctx.pads[:PLAYERS]
    devices = {f"device{n}": p.guid.hex() for n, p in enumerate(pads)}
    stale = [k for k in ini_section(text, SDLB) if k.startswith("device") and k not in devices]
    text = ini_rewrite(text, SDLB, {}, drop=stale)
    text = _replace_bindings(text, SDLB, bindings(lead, ctx), devices)
    seen = set()
    for pad in pads:
        if pad.guid in seen:
            continue
        seen.add(pad.guid)
        values = bindings(pad, ctx)
        text = _replace_bindings(text, f"gba.input-profile.{pad.guid.hex()}", values)
        if pad.joystick_name:
            text = _replace_bindings(text, f"gba.input-profile.{pad.joystick_name}", values)
    files = {path: text}
    qt = config_dir() / "qt.ini"
    if qt.is_file():
        files[qt] = _free_shortcuts(qt.read_text(), pads)
    return files
