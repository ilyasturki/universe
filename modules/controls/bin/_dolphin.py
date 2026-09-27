import os
import re
from pathlib import Path

from _controls import EAST, NORTH, SOUTH, WEST, Context, Skip, config_home, data_home, ini_rewrite, ini_section, ordinals

PORTS = 4
GC_CONTROLLER, WIIU_ADAPTER = "6", "12"
# Dolphin.ini's [SDL_Hints] as Dolphin fills it on first start.
SDL_HINTS = {
    "SDL_JOYSTICK_ENHANCED_REPORTS": "1",
    "SDL_JOYSTICK_WGI": "0",
    "SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED": "0",
    "SDL_JOYSTICK_DIRECTINPUT": "1",
    "SDL_JOYSTICK_HIDAPI_COMBINE_JOY_CONS": "1",
    "SDL_JOYSTICK_HIDAPI_VERTICAL_JOY_CONS": "0",
}
FACE_NAME = {SOUTH: "`Button S`", EAST: "`Button E`", WEST: "`Button W`", NORTH: "`Button N`"}
CALIBRATION = "100.00 141.42 100.00 141.42 100.00 141.42 100.00 141.42"
OWN_PROFILE = "universe-"
# A setting's value is a number or a flag; anything else is a binding expression the module replaces.
SETTING = re.compile(r"^(-?[0-9.]+( -?[0-9.]+)*|True|False)$")


def _stick(prefix, side):
    return {
        f"{prefix}/Up": f"`{side} Y+`",
        f"{prefix}/Down": f"`{side} Y-`",
        f"{prefix}/Left": f"`{side} X-`",
        f"{prefix}/Right": f"`{side} X+`",
    }


def _dpad(prefix):
    return {f"{prefix}/Up": "`Pad N`", f"{prefix}/Down": "`Pad S`", f"{prefix}/Left": "`Pad W`", f"{prefix}/Right": "`Pad E`"}


GCPAD = {
    "Buttons/A": "`Button S`",
    "Buttons/B": "`Button E`",
    "Buttons/X": "`Button W`",
    "Buttons/Y": "`Button N`",
    "Buttons/Z": "`Shoulder R`",
    "Buttons/Start": "Start",
    **_stick("Main Stick", "Left"),
    **_stick("C-Stick", "Right"),
    "Triggers/L": "`Trigger L`",
    "Triggers/R": "`Trigger R`",
    "Triggers/L-Analog": "`Trigger L`",
    "Triggers/R-Analog": "`Trigger R`",
    **_dpad("D-Pad"),
    "Rumble/Motor": "Motor",
}
GCPAD_DEFAULTS = {"Main Stick/Calibration": CALIBRATION, "C-Stick/Calibration": CALIBRATION}

IMU = {
    "IMUAccelerometer/Up": "`Accel Up`",
    "IMUAccelerometer/Down": "`Accel Down`",
    "IMUAccelerometer/Left": "`Accel Left`",
    "IMUAccelerometer/Right": "`Accel Right`",
    "IMUAccelerometer/Forward": "`Accel Forward`",
    "IMUAccelerometer/Backward": "`Accel Backward`",
    "IMUGyroscope/Pitch Up": "`Gyro Pitch Up`",
    "IMUGyroscope/Pitch Down": "`Gyro Pitch Down`",
    "IMUGyroscope/Roll Left": "`Gyro Roll Left`",
    "IMUGyroscope/Roll Right": "`Gyro Roll Right`",
    "IMUGyroscope/Yaw Left": "`Gyro Yaw Left`",
    "IMUGyroscope/Yaw Right": "`Gyro Yaw Right`",
    "IMUIR/Recenter": "`Thumb R`",
}


def user_dirs():
    """(config dir, GameSettings dir)."""
    root = os.environ.get("DOLPHIN_EMU_USERPATH")
    if not root and (Path.home() / ".dolphin-emu").is_dir():
        root = str(Path.home() / ".dolphin-emu")
    if root:
        return Path(root) / "Config", Path(root) / "GameSettings"
    return config_home() / "dolphin-emu", data_home() / "dolphin-emu" / "GameSettings"


def _read(path):
    try:
        return path.read_text()
    except OSError:
        return ""


def hints():
    dolphin = _read(user_dirs()[0] / "Dolphin.ini")
    core = ini_section(dolphin, "Core")
    adapter = any(core.get(f"SIDevice{n}") == WIIU_ADAPTER for n in range(PORTS))
    return {**SDL_HINTS, "SDL_JOYSTICK_HIDAPI_GAMECUBE": "0" if adapter else "1", **ini_section(dolphin, "SDL_Hints")}


def devices(pads):
    return [f"SDL/{n}/{p.name}" for n, p in zip(ordinals([p.name for p in pads]), pads, strict=True)]


def wiimote(ctx: Context, pad):
    keys = {
        "Buttons/A": "`Button S`",
        "Buttons/B": "`Trigger R`",
        "Buttons/1": "`Button W`",
        "Buttons/2": "`Button N`",
        "Buttons/-": "Back",
        "Buttons/+": "Start",
        **({"Buttons/Home": "Guide"} if ctx.guide else {}),
        **_dpad("D-Pad"),
        "Shake/X": "`Shoulder R`",
        "Shake/Y": "`Shoulder R`",
        "Shake/Z": "`Shoulder R`",
        "Rumble/Motor": "Motor",
        "Nunchuk/Buttons/C": "`Shoulder L`",
        "Nunchuk/Buttons/Z": "`Trigger L`",
        **_stick("Nunchuk/Stick", "Left"),
        **{f"Classic/Buttons/{k.upper()}": FACE_NAME[b] for k, b in ctx.face.items()},
        "Classic/Buttons/ZL": "`Trigger L`",
        "Classic/Buttons/ZR": "`Trigger R`",
        "Classic/Buttons/-": "Back",
        "Classic/Buttons/+": "Start",
        **({"Classic/Buttons/Home": "Guide"} if ctx.guide else {}),
        **_stick("Classic/Left Stick", "Left"),
        **_stick("Classic/Right Stick", "Right"),
        "Classic/Triggers/L": "`Shoulder L`",
        "Classic/Triggers/R": "`Shoulder R`",
        **_dpad("Classic/D-Pad"),
    }
    defaults = {"Extension": "Nunchuk", "Nunchuk/Stick/Calibration": CALIBRATION}
    if pad.gyro:
        keys.update(IMU)
    else:
        keys.update(_stick("IR", "Right"))
        defaults["IR/Relative Input"] = "True"
    return keys, defaults


def _section(text, name, device, keys, defaults):
    existing = ini_section(text, name)
    kept = {k: v for k, v in existing.items() if k in ("Extension", "Source") or SETTING.match(v)}
    values = {"Device": device, **kept, **keys, **{k: v for k, v in defaults.items() if k not in existing}}
    lines = {k: f"{k} = {v}" for k, v in values.items()}
    return ini_rewrite(text, name, lines, drop=set(existing) - set(values))


def _own_profile(section):
    return "".join(["[Profile]\n", *(f"{k} = {v}\n" for k, v in section.items() if k != "Source")])


def _on_pad(out, path, device):
    profile = out.get(path) or _read(path)
    if not ini_section(profile, "Profile").get("Device", "").startswith("SDL/"):
        return None
    return ini_rewrite(profile, "Profile", {"Device": f"Device = {device}"})


def _profiles(config, games, pads, device_names, sections):
    out = {}
    for game in sorted(games.glob("*.ini")) if games.is_dir() else ():
        text = _read(game)
        controls = ini_section(text, "Controls")
        lines, drop = {}, set()
        for kind, folder in (("PadProfile", "GCPad"), ("WiimoteProfile", "Wiimote")):
            theirs = {n: name for n in range(1, PORTS + 1) if (name := controls.get(f"{kind}{n}")) and not name.startswith(OWN_PROFILE)}
            for n in range(1, min(len(pads), PORTS) + 1):
                key, name = f"{kind}{n}", controls.get(f"{kind}{n}")
                if n in theirs:
                    path = config / "Profiles" / folder / f"{name}.ini"
                    if (profile := _on_pad(out, path, device_names[n - 1])) is not None:
                        out[path] = profile
                    continue
                # Dolphin's InputConfig::LoadConfig reads every port after a profiled one from that profile's file, so it gets no bindings.
                source = max((k for k in theirs if k < n), default=None)
                if source is None:
                    if name:
                        drop.add(key)
                    continue
                own = f"{OWN_PROFILE}{theirs[source]}-{n}"
                if name != own:
                    lines[key] = f"{key} = {own}"
                path = config / "Profiles" / folder / f"{theirs[source]}.ini"
                profile = _on_pad(out, path, device_names[n - 1])
                out[path.with_name(f"{own}.ini")] = _own_profile(sections[folder][n]) if profile is None else profile
        if lines or drop:
            out[game] = ini_rewrite(text, "Controls", lines, drop=drop)
    return out


def plan(ctx: Context):
    config, games = user_dirs()
    if not (config / "Dolphin.ini").is_file():
        raise Skip(f"no Dolphin.ini in {config}: start Dolphin once, then its controls are written")
    pads = ctx.pads[:PORTS]
    names = devices(pads)
    platform = ctx.platform
    gamecube, wii = "GameCube" in platform or not platform, "Wii" in platform or not platform

    gcpad = _read(config / "GCPadNew.ini")
    wiimotes = _read(config / "WiimoteNew.ini")
    dolphin = _read(config / "Dolphin.ini")
    core = ini_section(dolphin, "Core")
    for n, (pad, device) in enumerate(zip(pads, names, strict=True)):
        gcpad = _section(gcpad, f"GCPad{n + 1}", device, GCPAD, GCPAD_DEFAULTS)
        section = f"Wiimote{n + 1}"
        keys, defaults = wiimote(ctx, pad)
        if wii and ini_section(wiimotes, section).get("Source", "1" if n == 0 else "0") == "0":
            keys["Source"] = "1"
        wiimotes = _section(wiimotes, section, device, keys, defaults)
        if gamecube and core.get(f"SIDevice{n}", GC_CONTROLLER if n == 0 else "0") == "0":
            dolphin = ini_rewrite(dolphin, "Core", {f"SIDevice{n}": f"SIDevice{n} = {GC_CONTROLLER}"})

    out = {config / "GCPadNew.ini": gcpad, config / "WiimoteNew.ini": wiimotes, config / "Dolphin.ini": dolphin}
    sections = {
        "GCPad": {n: ini_section(gcpad, f"GCPad{n}") for n in range(1, len(pads) + 1)},
        "Wiimote": {n: ini_section(wiimotes, f"Wiimote{n}") for n in range(1, len(pads) + 1)},
    }
    out.update(_profiles(config, games, pads, names, sections))
    return out
