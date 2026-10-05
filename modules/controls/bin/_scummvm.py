from pathlib import Path

from _controls import Context, config_home, first_pad, ini_rewrite, ini_section

SECTION = "scummvm"


def plan(ctx: Context):
    """`joystick_num` -1 turns the joystick off; unset means index 0."""
    index = first_pad(ctx).index
    legacy = Path.home() / ".scummvmrc"
    path = legacy if legacy.is_file() else config_home() / "scummvm" / "scummvm.ini"
    text = path.read_text() if path.is_file() else ""
    current = ini_section(text, SECTION).get("joystick_num")
    if current == "-1" or current == str(index) or (current is None and index == 0):
        return {}
    return {path: ini_rewrite(text, SECTION, {"joystick_num": f"joystick_num={index}"})}
