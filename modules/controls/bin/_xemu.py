from pathlib import Path

from _controls import Context, Skip, data_home, ini_rewrite, ini_section

TABLE = "input.bindings"
PORTS = 4
DUKE = "usb-xbox-gamepad"


def config_path(runner_path=""):
    if runner_path:
        portable = Path(runner_path).resolve().parent / "xemu.toml"
        if portable.is_file():
            return portable
    return data_home() / "xemu" / "xemu" / "xemu.toml"


def plan(ctx: Context):
    path = config_path(ctx.runner_path)
    if not path.is_file():
        raise Skip("no xemu.toml: start xemu once, then its controls are written")
    text = path.read_text()
    current = {k: v.strip().strip("'\"") for k, v in ini_section(text, TABLE).items()}
    pads = ctx.pads[:PORTS]
    values = {}
    for n in range(1, PORTS + 1):
        if n <= len(pads):
            values[f"port{n}"] = pads[n - 1].guid.hex()
            values[f"port{n}_driver"] = current.get(f"port{n}_driver") or DUKE
        elif current.get(f"port{n}") != "keyboard":
            # A port keeps the GUID it last saw and waits for that pad, holding a later pad off it.
            values[f"port{n}"] = ""
    return {path: ini_rewrite(text, TABLE, {k: f"{k} = '{v}'" for k, v in values.items()})}
