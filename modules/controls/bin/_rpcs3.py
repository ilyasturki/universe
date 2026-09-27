import json
import os
import re
from pathlib import Path

from _controls import Context, Skip, config_home, ordinals

PLAYERS = 7
# RPCS3's SDL handler default, less `PS Button`, which the writer decides.
CONFIG = {
    "Left Stick Left": "LS X-",
    "Left Stick Down": "LS Y-",
    "Left Stick Right": "LS X+",
    "Left Stick Up": "LS Y+",
    "Right Stick Left": "RS X-",
    "Right Stick Down": "RS Y-",
    "Right Stick Right": "RS X+",
    "Right Stick Up": "RS Y+",
    "Start": "Start",
    "Select": "Back",
    "Square": "West",
    "Cross": "South",
    "Circle": "East",
    "Triangle": "North",
    "Left": "Left",
    "Down": "Down",
    "Right": "Right",
    "Up": "Up",
    "R1": "RB",
    "R2": "RT",
    "R3": "RS",
    "L1": "LB",
    "L2": "LT",
    "L3": "LS",
}
# The PS3 controller the settings dialog writes as the emulated device.
DEVICE = {"Device Class Type": 0, "Vendor ID": 1356, "Product ID": 616}
PLAYER = re.compile(r"^Player ([1-7]) Input:\s*$")
FIELD = re.compile(r"^(\s+)(Handler|Device):\s*(.*?)\s*$")


def root(runner_path=""):
    if runner_path:
        portable = Path(os.path.realpath(runner_path)).parent / "portable"
        if portable.is_dir():
            return portable
    return config_home() / "rpcs3"


def hints():
    return {"SDL_JOYSTICK_HIDAPI_PS3": "1"}


def databases():
    db = root() / "input_configs" / "gamecontrollerdb.txt"
    return [db] if db.is_file() else []


def devices(pads):
    return [f"{p.name} {n + 1}" for n, p in zip(ordinals([p.name for p in pads]), pads, strict=True)]


def _unquote(value):
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return json.loads(value) if value[0] == '"' else value[1:-1]
    return value


def active_name(input_configs):
    try:
        text = (input_configs / "active_input_configurations.yml").read_text()
    except OSError:
        return "Default"
    m = re.search(r"^\s+global:\s*(.+?)\s*$", text, re.MULTILINE)
    return _unquote(m.group(1)) if m else "Default"


def render(ctx: Context):
    q = json.dumps
    out = []
    for n, device in enumerate(devices(ctx.pads[:PLAYERS]), 1):
        out += [f"Player {n} Input:", "  Handler: SDL", f"  Device: {q(device)}", "  Config:"]
        out += [f"    {k}: {q(v)}" for k, v in {**CONFIG, "PS Button": "Guide" if ctx.guide else ""}.items()]
        out += [f"    {k}: {v}" for k, v in DEVICE.items()]
        out.append(f"  Buddy Device: {q('')}")
    for n in range(len(ctx.pads[:PLAYERS]) + 1, PLAYERS + 1):
        out += [f"Player {n} Input:", f"  Handler: {q('Null')}", f"  Device: {q('Null')}", f"  Buddy Device: {q('')}"]
    return "\n".join(out) + "\n"


def repoint(text, names):
    lines = text.splitlines()
    player, handler, device_line = None, None, None

    def flush():
        if player is not None and handler == "SDL" and device_line is not None and player <= len(names):
            m = FIELD.match(lines[device_line])
            indent = m.group(1) if m else "  "
            lines[device_line] = f"{indent}Device: {json.dumps(names[player - 1])}"

    for i, line in enumerate(lines):
        m = PLAYER.match(line)
        if m:
            flush()
            player, handler, device_line = int(m.group(1)), None, None
            continue
        f = FIELD.match(line)
        if player is not None and f and len(f.group(1)) == 2:
            if f.group(2) == "Handler":
                handler = _unquote(f.group(3))
            else:
                device_line = i
    flush()
    return "\n".join(lines) + ("\n" if text.endswith("\n") else "")


def plan(ctx: Context):
    base = root(ctx.runner_path)
    if not base.is_dir():
        raise Skip("no RPCS3 settings yet: start RPCS3 once, then its controls are written")
    input_configs = base / "input_configs"
    target = input_configs / "global" / f"{active_name(input_configs)}.yml"
    names = devices(ctx.pads[:PLAYERS])
    files = {target: render(ctx)}
    others = [*(input_configs / "global").glob("*.yml"), *input_configs.glob("*/Default.yml")] if input_configs.is_dir() else []
    for path in sorted(set(others)):
        if path == target:
            continue
        text = path.read_text()
        new = repoint(text, names)
        if new != text:
            files[path] = new
    return files
