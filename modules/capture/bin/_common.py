"""Shared helpers for capture module hooks. Not a hook itself."""
import glob
import json
import os
import re
import subprocess
import sys

# Overridden by GSR's -ffmpeg-video-opts last, right before avcodec_open2, so
# these keys win over GSR's own choices even though -bm cbr is also set.
QVBR_OPTS = "rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000"

EXTENSION_UUID = "universe@ilyasturki.github.io"
WINDOWS_BUS_NAME = "org.universe.Windows"

# org.gnome.Mutter.ScreenCast RecordWindow cursor-mode (meta-screen-cast.h)
CURSOR_HIDDEN = 0
CURSOR_EMBEDDED = 1

# codec setting -> (VA GStreamer encoder, raw format vapostproc must output)
VA_ENCODERS = {
    "av1_10bit": ("vaav1enc", "P010_10LE"),
    "av1": ("vaav1enc", "NV12"),
    "hevc": ("vah265enc", "NV12"),
    "h264": ("vah264enc", "NV12"),
}
VA_PARSERS = {"vaav1enc": "av1parse", "vah265enc": "h265parse", "vah264enc": "h264parse"}

SETTINGS_DEFAULTS = {
    "enabled": True,
    "source": "window",
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


def gsr_args(settings, screen, output_path):
    """The gpu-screen-recorder argv (no systemd-run wrapper) — the screen-capture path."""
    cursor = "yes" if settings.get("cursor") else "no"
    fps = str(resolve_fps(settings.get("fps", 60), screen))
    codec = settings.get("codec", "av1_10bit")
    return [
        "gpu-screen-recorder",
        "-w", screen,
        "-cursor", cursor,
        "-f", fps,
        "-fm", "vfr",
        "-c", "mkv",
        "-k", codec,
        "-ac", "opus",
        "-tune", "quality",
        "-bm", "cbr",
        "-q", "20000",
        *audio_args(settings.get("audio")),
        "-ffmpeg-video-opts", quality_opts(settings.get("quality")),
        "-o", output_path,
    ]


def cursor_mode(settings):
    return CURSOR_EMBEDDED if settings.get("cursor") else CURSOR_HIDDEN


def va_rate_control_args(quality):
    # VA encoders take no ffmpeg -ffmpeg-video-opts string; a raw value is dropped.
    if quality and quality != "qvbr":
        log(f"quality {quality!r} is a raw ffmpeg string, ignored for window capture; using qvbr")
    return ["rate-control=vbr", "bitrate=16000", "target-percentage=50"]


def audio_gst_branches(setting):
    if setting == "none":
        return []
    if setting == "output":
        devices = ["@DEFAULT_MONITOR@"]
    else:
        if setting != "output+input":
            log(f"unknown audio setting {setting!r}, using output+input")
        devices = ["@DEFAULT_MONITOR@", "@DEFAULT_SOURCE@"]
    branch = []
    for device in devices:
        branch += [
            "pulsesrc", f"device={device}", "do-timestamp=true", "!",
            "audioconvert", "!", "audioresample", "!", "audio/x-raw,channels=2", "!",
            "opusenc", "!", "queue", "!", "mux.",
        ]
    return branch


def gst_window_args(node_id, settings, fps, output_path):
    """gst-launch argv that encodes a PipeWire window node to mkv. fps None keeps the source rate."""
    codec = settings.get("codec", "av1_10bit")
    if codec not in VA_ENCODERS:
        log(f"unknown codec {codec!r}, using av1_10bit")
        codec = "av1_10bit"
    encoder, fmt = VA_ENCODERS[codec]
    parser = VA_PARSERS[encoder]
    args = [
        "gst-launch-1.0", "-e",
        "pipewiresrc", f"path={node_id}", "do-timestamp=true", "!",
        "video/x-raw(ANY)", "!", "queue", "!",
    ]
    if fps is not None:
        args += ["videorate", "drop-only=true", f"max-rate={fps}", "!"]
    args += [
        "vapostproc", "!", f"video/x-raw(memory:VAMemory),format={fmt}", "!",
        encoder, *va_rate_control_args(settings.get("quality")), "!",
        parser, "!", "queue", "!",
        "matroskamux", "name=mux", "!",
        "filesink", f"location={output_path}",
    ]
    args += audio_gst_branches(settings.get("audio"))
    return args


def unit_cgroup(unit):
    if not unit:
        return None
    out = subprocess.run(
        ["systemctl", "--user", "show", "-p", "ControlGroup", "--value", unit],
        capture_output=True, text=True,
    ).stdout.strip()
    return out or None


def cgroup_matches(proc_cgroup_text, unit_cg):
    """Whether a /proc/<pid>/cgroup dump belongs to (or under) the unit's cgroup path."""
    if not unit_cg:
        return False
    base = unit_cg.rstrip("/")
    for line in proc_cgroup_text.splitlines():
        path = line.split("::", 1)[1] if "::" in line else line.rsplit(":", 1)[-1]
        if path == base or path.startswith(base + "/"):
            return True
    return False


def pid_in_unit(pid, unit_cg):
    try:
        with open(f"/proc/{pid}/cgroup") as f:
            text = f.read()
    except OSError:
        return False
    return cgroup_matches(text, unit_cg)


def segment_path(pending_dir, session_id, index):
    return os.path.join(pending_dir, f"{session_id}-{index}.mkv")


def _segment_index(session_id, path):
    m = re.search(rf"{re.escape(session_id)}-(\d+)\.mkv$", os.path.basename(path))
    return int(m.group(1)) if m else -1


def segment_paths(pending_dir, session_id):
    found = glob.glob(os.path.join(pending_dir, f"{session_id}-*.mkv"))
    numbered = [p for p in found if _segment_index(session_id, p) >= 0]
    return sorted(numbered, key=lambda p: _segment_index(session_id, p))


def concat_list_lines(paths):
    # ffmpeg concat demuxer: one `file '<abs path>'` per line, single quotes escaped.
    return [f"file '{os.path.abspath(p).replace(chr(39), chr(39) + chr(92) + chr(39) + chr(39))}'" for p in paths]


def extension_installed(uuid=EXTENSION_UUID):
    home = os.path.join(os.path.expanduser("~"), ".local/share/gnome-shell/extensions", uuid)
    if os.path.isdir(home):
        return True
    dirs = os.environ.get("XDG_DATA_DIRS") or "/usr/share"
    return any(d and os.path.isdir(os.path.join(d, "gnome-shell/extensions", uuid))
               for d in dirs.split(":"))


def is_gnome():
    return "GNOME" in (os.environ.get("XDG_CURRENT_DESKTOP") or "")


def enable_extension(uuid=EXTENSION_UUID):
    subprocess.run(
        ["busctl", "--user", "call", "org.gnome.Shell.Extensions",
         "/org/gnome/Shell/Extensions", "org.gnome.Shell.Extensions", "EnableExtension", "s", uuid],
        capture_output=True, text=True,
    )


def bus_name_has_owner(name):
    r = subprocess.run(
        ["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus",
         "org.freedesktop.DBus", "NameHasOwner", "s", name],
        capture_output=True, text=True,
    )
    return r.returncode == 0 and r.stdout.strip() == "b true"


def universe_bin():
    return os.environ.get("UNIVERSE_BIN") or "universe"
