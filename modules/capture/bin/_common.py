"""Shared helpers for capture module hooks. Not a hook itself."""
import json
import os
import subprocess
import sys

# Overridden by GSR's -ffmpeg-video-opts last, right before avcodec_open2, so
# these keys win over GSR's own choices even though -bm cbr is also set.
QVBR_OPTS = "rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000"

SETTINGS_DEFAULTS = {
    "enabled": True,
    "cursor": False,
    "codec": "av1_10bit",
    "fps": 60,
    "audio": "output+input",
    "min_duration_s": 240,
    "quality": "qvbr",
}


def log(msg):
    print(f"[capture] {msg}", file=sys.stderr, flush=True)


def load_settings():
    raw = os.environ.get("MODULE_SETTINGS_JSON", "") or "{}"
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as e:
        log(f"MODULE_SETTINGS_JSON invalid ({e}), using defaults")
        parsed = {}
    settings = dict(SETTINGS_DEFAULTS)
    settings.update(parsed)
    return settings


def resolve_screen():
    screen = (os.environ.get("SESSION_SCREEN") or "").strip()
    if screen:
        return screen
    out = subprocess.run(
        ["gpu-screen-recorder", "--list-monitors"],
        capture_output=True, text=True, check=True,
    ).stdout
    for line in out.splitlines():
        line = line.strip()
        if line:
            return line.split("|", 1)[0]
    raise RuntimeError("gpu-screen-recorder --list-monitors returned no monitor")


def screen_refresh_hz(screen):
    """The current mode's refresh rate of a DRM connector, from Mutter's DisplayConfig; None off GNOME."""
    try:
        out = subprocess.run(
            ["busctl", "--user", "--json=short", "call", "org.gnome.Mutter.DisplayConfig",
             "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig", "GetCurrentState"],
            capture_output=True, text=True, check=True, timeout=5,
        ).stdout
        monitors = json.loads(out)["data"][1]
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, IndexError, TypeError):
        return None
    for monitor in monitors:
        if monitor[0][0] != screen:
            continue
        for mode in monitor[1]:
            props = mode[6] if len(mode) > 6 and isinstance(mode[6], dict) else {}
            if (props.get("is-current") or {}).get("data") is True:
                return round(float(mode[3]))
    return None


def resolve_fps(setting, screen):
    if setting != "auto":
        try:
            return int(setting)
        except (TypeError, ValueError):
            log(f"fps {setting!r} is not a number, using {SETTINGS_DEFAULTS['fps']}")
            return SETTINGS_DEFAULTS["fps"]
    hz = screen_refresh_hz(screen)
    if hz is None:
        log(f"fps auto: no refresh rate for {screen}, using {SETTINGS_DEFAULTS['fps']}")
        return SETTINGS_DEFAULTS["fps"]
    return hz


def audio_args(setting):
    if setting == "output":
        return ["-a", "default_output"]
    if setting == "none":
        return []
    if setting != "output+input":
        log(f"unknown audio setting {setting!r}, using output+input")
    return ["-a", "default_output", "-a", "default_input"]


def quality_opts(setting):
    if not setting or setting == "qvbr":
        return QVBR_OPTS
    return setting


def universe_bin():
    return os.environ.get("UNIVERSE_BIN") or "universe"
