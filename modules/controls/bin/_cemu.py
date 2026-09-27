import xml.etree.ElementTree as ET
from html import escape
from pathlib import Path

from _controls import EAST, GUIDE, NORTH, SOUTH, WEST, Context, Skip, config_home, ini_section, ordinals
from _gamepads import LABEL_B

# Cemu's SDL init; the PS4/PS5 rumble pair reaches SDL3 as ENHANCED_REPORTS through sdl2-compat.
HINTS = {
    "SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS": "1",
    "SDL_JOYSTICK_HIDAPI_PS4": "1",
    "SDL_JOYSTICK_HIDAPI_PS5": "1",
    "SDL_JOYSTICK_ENHANCED_REPORTS": "1",
    "SDL_JOYSTICK_HIDAPI_GAMECUBE": "1",
    "SDL_JOYSTICK_HIDAPI_SWITCH": "1",
    "SDL_JOYSTICK_HIDAPI_JOY_CONS": "1",
    "SDL_JOYSTICK_HIDAPI_STADIA": "1",
    "SDL_JOYSTICK_HIDAPI_STEAM": "1",
    "SDL_JOYSTICK_HIDAPI_LUNA": "1",
}
PLAYERS = 8
GAMEPAD, PRO = "Wii U GamePad", "Wii U Pro Controller"

# Cemu's controller-side ids past the SDL button enum.
ZL, ZR = 42, 43
LX_P, LY_P, RX_P, RY_P, LX_N, LY_N, RX_N, RY_N = 38, 39, 40, 41, 44, 45, 46, 47
# Emulated ids after A1 B2 X3 Y4, per type; Cemu's own SDL default, stick up on the negative Y (SDL's sense).
REST = {
    GAMEPAD: {5: 9, 6: 10, 7: ZL, 8: ZR, 9: 6, 10: 4, 11: 11, 12: 12, 13: 13, 14: 14, 15: 7, 16: 8,
              17: LY_N, 18: LY_P, 19: LX_N, 20: LX_P, 21: RY_N, 22: RY_P, 23: RX_N, 24: RX_P},
    PRO: {5: 9, 6: 10, 7: ZL, 8: ZR, 9: 6, 10: 4, 12: 11, 13: 12, 14: 13, 15: 14, 16: 7, 17: 8,
          18: LY_N, 19: LY_P, 20: LX_N, 21: LX_P, 22: RY_N, 23: RY_P, 24: RX_N, 25: RX_P},
}  # fmt: skip
HOME = {GAMEPAD: 27, PRO: 11}
TUNING = ("rumble", "axis", "rotation", "trigger")
# sdl2-compat hands a pad labelled B on SOUTH to Cemu's gamepad calls with SOUTH/EAST and WEST/NORTH swapped.
LABEL_SWAP = {SOUTH: EAST, EAST: SOUTH, WEST: NORTH, NORTH: WEST}


def hints():
    return dict(HINTS)


def uuids(pads):
    keys = [p.guid.hex() for p in pads]
    return [f"{n}_{g}" for n, g in zip(ordinals(keys), keys, strict=True)]


def mappings(ctx: Context, pad, kind):
    swap = pad.labels[0] == LABEL_B
    out = {i: (LABEL_SWAP[b] if swap else b) for i, b in zip((1, 2, 3, 4), (ctx.face[k] for k in "abxy"), strict=True)}
    out.update(REST[kind])
    if ctx.guide:
        out[HOME[kind]] = GUIDE
    return out


def _load(path):
    try:
        return ET.parse(path).getroot()
    except (OSError, ET.ParseError):
        return None


def _tuning(root):
    controller = root.find("controller") if root is not None else None
    if controller is None:
        return ""
    kept = [el for name in TUNING for el in controller.findall(name)]
    for el in kept:
        el.tail = None
    return "\n\t\t".join(ET.tostring(el, encoding="unicode") for el in kept)


def profile(ctx: Context, pad, uuid, player, existing):
    kind = existing.findtext("type") if existing is not None else None
    if kind not in REST:
        kind = GAMEPAD if player == 0 else PRO
    name = existing.findtext("profile") if existing is not None else None
    entries = "".join(
        f"\t\t\t<entry>\n\t\t\t\t<mapping>{m}</mapping>\n\t\t\t\t<button>{b}</button>\n\t\t\t</entry>\n" for m, b in mappings(ctx, pad, kind).items()
    )
    tuning = _tuning(existing)
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n<emulated_controller>\n'
        f"\t<type>{kind}</type>\n"
        + (f"\t<profile>{escape(name, quote=False)}</profile>\n" if name else "")
        + ("\t<toggle_display>0</toggle_display>\n" if kind == GAMEPAD else "")
        + "\t<controller>\n\t\t<api>SDLController</api>\n"
        f"\t\t<uuid>{uuid}</uuid>\n\t\t<display_name>{escape(pad.name, quote=False)}</display_name>\n"
        + ("\t\t<motion>true</motion>\n" if pad.gyro else "")
        + (f"\t\t{tuning}\n" if tuning else "")
        + f"\t\t<mappings>\n{entries}\t\t</mappings>\n\t</controller>\n</emulated_controller>\n"
    )


def repoint(path, pad, uuid):
    try:
        text = path.read_text()
    except OSError:
        return None
    root = _load(path)
    controller = root.find("controller") if root is not None else None
    if controller is None or controller.findtext("api") != "SDLController":
        return None
    for tag, value in (("uuid", uuid), ("display_name", escape(pad.name, quote=False))):
        start, end = text.find(f"<{tag}>"), text.find(f"</{tag}>")
        if start < 0 or end < start:
            return None
        text = text[: start + len(tag) + 2] + value + text[end:]
    return text


def plan(ctx: Context):
    portable = Path(ctx.runner_path).parent / "portable" if ctx.runner_path else None
    config = portable if portable and portable.is_dir() else config_home() / "Cemu"
    if not config.is_dir():
        raise Skip(f"no {config}: start Cemu once, then its controls are written")
    profiles = config / "controllerProfiles"
    pads = [p for p in ctx.pads if p.gamepad_type][:PLAYERS]
    ids = uuids(pads)
    out = {}
    for player, (pad, uuid) in enumerate(zip(pads, ids, strict=True)):
        path = profiles / f"controller{player}.xml"
        out[path] = profile(ctx, pad, uuid, player, _load(path))
    games = config / "gameProfiles"
    for game in sorted(games.glob("*.ini")) if games.is_dir() else ():
        controller = ini_section(game.read_text(errors="replace"), "Controller")
        for n in range(1, PLAYERS + 1):
            name = controller.get(f"controller{n}")
            if not name or n > len(pads):
                continue
            path = profiles / f"{name}.xml"
            text = repoint(path, pads[n - 1], ids[n - 1])
            if text is not None:
                out[path] = text
    return out
