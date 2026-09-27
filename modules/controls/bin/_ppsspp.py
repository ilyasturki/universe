from _controls import Context, Skip, config_home, edit_section, first_file, key_of, sections, value_of
from _gamepads import LABEL_B

SECTION = "ControlMapping"
# PPSSPP's device ids for SDL pads: 10 + the SDL2 device index.
PAD_IDS = range(10, 20)
# SDL2 A, B, X, Y as PPSSPP's NKCODE_BUTTON_2, _3, _4, _1.
A, B, X, Y = 189, 190, 191, 188
# NKCODE_BACK: guide left unbound opens PPSSPP's pause menu.
GUIDE = 4
KEYS = {
    "Up": 19,
    "Down": 20,
    "Left": 21,
    "Right": 22,
    "Start": 197,
    "Select": 196,
    "L": 193,
    "R": 192,
    "An.Up": 4003,
    "An.Down": 4002,
    "An.Left": 4001,
    "An.Right": 4000,
    "RightAn.Up": 4007,
    "RightAn.Down": 4006,
    "RightAn.Left": 4005,
    "RightAn.Right": 4004,
}


def face(pad):
    """sdl2-compat hands PPSSPP the button labelled A, so a Nintendo pad's labels are swapped back to Cross on SOUTH."""
    if pad.labels and pad.labels[0] == LABEL_B:
        return {"Cross": B, "Circle": A, "Square": Y, "Triangle": X}
    return {"Cross": A, "Circle": B, "Square": X, "Triangle": Y}


def _pad_part(part):
    device = part.split("-", 1)[0]
    return device.lstrip("-").isdigit() and int(device) in PAD_IDS


def without_pads(value):
    return [m for m in (v.strip() for v in value.split(",")) if m and not any(_pad_part(p) for p in m.split(":"))]


def rebind(lines, device, pad, guide=False):
    ours = {k: [f"{device}-{code}"] for k, code in {**face(pad), **KEYS}.items()}
    if not guide:
        ours["Home"] = [f"{device}-{GUIDE}"]
    out, done = [], set()
    for line in lines:
        key = key_of(line)
        if key is None:
            out.append(line)
            continue
        mappings = without_pads(value_of(line)) + ours.get(key, [])
        done.add(key)
        if mappings:
            out.append(f"{key} = {','.join(mappings)}")
    return out + [f"{k} = {','.join(v)}" for k, v in ours.items() if k not in done]


def plan(ctx: Context):
    path = first_file(config_home() / "ppsspp" / "PSP" / "SYSTEM" / "controls.ini")
    if path is None:
        raise Skip("no controls.ini yet: start PPSSPP once, then its controls are written")
    text = path.read_text()
    if SECTION not in sections(text):
        raise Skip("controls.ini has no [ControlMapping]: start PPSSPP once, then its controls are written")
    pad = ctx.pads[0]
    if 10 + pad.index not in PAD_IDS:
        raise Skip(f"{pad.name} is SDL joystick {pad.index}: PPSSPP numbers only the first ten")
    return {path: edit_section(text, SECTION, lambda lines: rebind(lines, 10 + pad.index, pad, ctx.guide))}
