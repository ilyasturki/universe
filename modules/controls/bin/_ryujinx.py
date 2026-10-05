import json

from _controls import EAST, NORTH, SOUTH, WEST, Context, Skip, config_home, ordinals, swap_names
from _gamepads import LABEL_A, LABEL_B, LABEL_CIRCLE, LABEL_CROSS, LABEL_SQUARE, LABEL_TRIANGLE, LABEL_X, LABEL_Y

PLAYERS = 8
# Ryujinx names a face button by the label SDL gives it (SDL3Gamepad re-points A/B/X/Y by SDL_GetGamepadButtonLabel), not by its position.
TOKEN = {LABEL_A: "A", LABEL_CROSS: "A", LABEL_B: "B", LABEL_CIRCLE: "B", LABEL_X: "X", LABEL_SQUARE: "X", LABEL_Y: "Y", LABEL_TRIANGLE: "Y"}
POSITION_TOKEN = {SOUTH: "A", EAST: "B", WEST: "X", NORTH: "Y"}
TUNING = ("deadzone_left", "deadzone_right", "range_left", "range_right", "trigger_threshold", "rumble", "led")


def hints():
    return {"SDL_JOYSTICK_HIDAPI_JOY_CONS": "1", "SDL_JOYSTICK_HIDAPI_COMBINE_JOY_CONS": "0", "SDL_JOYSTICK_ENHANCED_REPORTS": "1"}


def ryujinx_guid(g):
    """SDL3GamepadDriver's id: the GUID as .NET GUID text, its name CRC (bytes 2-3) zeroed."""
    return f"0000{g[1]:02x}{g[0]:02x}-{g[5]:02x}{g[4]:02x}-{g[6]:02x}{g[7]:02x}-{g[8]:02x}{g[9]:02x}-{g[10:16].hex()}"


def ids(pads):
    return [f"{n}-{ryujinx_guid(p.guid)}" for n, p in zip(ordinals(pads, lambda p: ryujinx_guid(p.guid)), pads, strict=True)]


def _face(pad, button):
    return TOKEN.get(pad.labels[button], POSITION_TOKEN[button])


def _stick(side, old):
    stick = {"joystick": side, "invert_stick_x": False, "invert_stick_y": False, "rotate90_cw": False, "stick_button": f"{side}Stick"}
    for k in ("invert_stick_x", "invert_stick_y", "rotate90_cw"):
        if isinstance(old.get(k), bool):
            stick[k] = old[k]
    return stick


def entry(pad, pad_id, player, ctx: Context, old):
    face = ctx.face
    motion = {"motion_backend": "GamepadDriver", "sensitivity": 100, "gyro_deadzone": 1, **(old.get("motion") or {}), "enable_motion": pad.gyro}
    out = {
        "left_joycon_stick": _stick("Left", old.get("left_joycon_stick") or {}),
        "right_joycon_stick": _stick("Right", old.get("right_joycon_stick") or {}),
        "deadzone_left": 0.1,
        "deadzone_right": 0.1,
        "range_left": 1,
        "range_right": 1,
        "trigger_threshold": 0.5,
        "motion": motion,
        "rumble": {"strong_rumble": 1, "weak_rumble": 1, "enable_rumble": False, "use_hdrumble": False},
        "led": {"enable_led": False, "turn_off_led": False, "use_rainbow": False, "led_color": 0},
        "left_joycon": {
            "button_minus": "Back",
            "button_l": "LeftShoulder",
            "button_zl": "LeftTrigger",
            "button_sl": "SingleLeftTrigger0",
            "button_sr": "SingleRightTrigger0",
            "dpad_up": "DpadUp",
            "dpad_down": "DpadDown",
            "dpad_left": "DpadLeft",
            "dpad_right": "DpadRight",
        },
        "right_joycon": {
            "button_plus": "Start",
            "button_r": "RightShoulder",
            "button_zr": "RightTrigger",
            "button_sl": "SingleLeftTrigger1",
            "button_sr": "SingleRightTrigger1",
            "button_x": _face(pad, face["x"]),
            "button_b": _face(pad, face["b"]),
            "button_y": _face(pad, face["y"]),
            "button_a": _face(pad, face["a"]),
        },
        "version": 1,
        "backend": "GamepadSDL3",
        "id": pad_id,
        "name": f"{pad.name} ({pad_id.split('-', 1)[0]})",
        "controller_type": "ProController",
        "player_index": f"Player{player}",
        "enable_dynamic_gamepad_swap": False,
    }
    for side in ("left_joycon", "right_joycon"):
        out[side] = swap_names(ctx, out[side], "LeftShoulder", "RightShoulder", "LeftTrigger", "RightTrigger")
    for k in TUNING:
        value = old.get(k)
        if isinstance(out[k], dict) and isinstance(value, dict):
            out[k] = {**out[k], **value}
        elif not isinstance(out[k], dict) and isinstance(value, int | float) and not isinstance(value, bool):
            out[k] = value
    return out


def rewrite(doc, ctx: Context):
    pads = ctx.pads[:PLAYERS]
    old = {e.get("player_index"): e for e in doc.get("input_config") or [] if isinstance(e, dict)}
    entries = [entry(pad, pad_id, n, ctx, old.get(f"Player{n}") or {}) for n, (pad, pad_id) in enumerate(zip(pads, ids(ctx.pads)[:PLAYERS], strict=True), 1)]
    doc["input_config"] = entries
    doc["player_input_assignments"] = [
        {"player_index": e["player_index"], "enable_dynamic_input_swap": False, "devices": [{"type": "Controller", "id": e["id"], "profile_name": None}]}
        for e in entries
    ]
    return doc


def _load(path):
    try:
        text = path.read_text()
        doc = json.loads(text)
    except (OSError, ValueError):
        return None, None
    if not isinstance(doc, dict) or not isinstance(doc.get("version"), int):
        return None, None
    return text, doc


def _dump(doc, like):
    return json.dumps(doc, indent=2, ensure_ascii=False) + ("\n" if like.endswith("\n") else "")


def plan(ctx: Context):
    base = config_home() / "Ryujinx"
    text, doc = _load(base / "Config.json")
    if doc is None:
        raise Skip("no readable Ryujinx Config.json: start Ryujinx once, then its controls are written")
    out = {base / "Config.json": _dump(rewrite(doc, ctx), text)}
    # A game's own Config.json replaces the global one, input included, unless it defers to the global input.
    for path in sorted((base / "games").glob("*/Config.json")):
        text, doc = _load(path)
        if doc is None or doc.get("use_input_global_config") is True:
            continue
        out[path] = _dump(rewrite(doc, ctx), text)
    return out
