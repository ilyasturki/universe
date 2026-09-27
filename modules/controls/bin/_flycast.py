from _controls import Context, Skip, config_home, ini_rewrite, ini_section

SECTION = "input"
PORTS = 4
SEGA_CONTROLLER = "0"
NONE = "10"
# Keyed by SDL instance ids, which the hook cannot predict: a stale one moves a pad to another port.
INSTANCE_PORT = "maple_sdl_joystick_"


def plan(ctx: Context):
    path = config_home() / "flycast" / "emu.cfg"
    if not path.is_file():
        raise Skip("no flycast/emu.cfg: start Flycast once, then its controls are written")
    text = path.read_text()
    current = ini_section(text, SECTION)
    stale = [k for k in current if k.startswith(INSTANCE_PORT)]
    devices = {f"device{n}": f"device{n} = {SEGA_CONTROLLER}" for n in range(2, min(PORTS, len(ctx.pads)) + 1) if current.get(f"device{n}", NONE) == NONE}
    if not stale and not devices:
        return {}
    return {path: ini_rewrite(text, SECTION, devices, drop=stale)}
