"""Shared helpers for capture module hooks. Not a hook itself."""
import json
import os
import re
import shlex
import subprocess
import sys
import time

# quality preset -> (AV1 quality factor 0-255, H.26x quality factor 0-51, target kbps, ceiling kbps).
# QVBR on both backends: constant quality up to the ceiling; very_high is the measured
# ~7-8 GB/h AMD configuration (uncapped vbr peaked at 420 Mbps, 18-21 GB/h).
QUALITY_PRESETS = {
    "medium": (150, 32, 6000, 12000),
    "high": (120, 27, 10000, 20000),
    "very_high": (95, 22, 16000, 32000),
    "ultra": (70, 17, 24000, 48000),
}
DEFAULT_QUALITY = "very_high"

EXTENSION_UUID = "universe@ilyasturki.github.io"
WINDOWS_BUS_NAME = "org.universe.Windows"

CONTAINERS = ("mkv", "mp4")
AUDIO_CODECS = ("opus", "aac", "flac")

SETTINGS_DEFAULTS = {
    "enabled": True,
    "source": "screen",
    "cursor": False,
    "codec": "av1_10bit",
    "quality": DEFAULT_QUALITY,
    "fps": 60,
    "size": "native",
    "container": "mkv",
    "audio": "output+input",
    "audio_codec": "opus",
    "audio_bitrate": "auto",
    "min_duration_s": 240,
    "window_wait_s": 60,
    "ffmpeg_video_opts": "",
    "gsr_extra_args": "",
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


def quality_preset(setting):
    """The preset a `quality` value names; "qvbr" (the old default) and an unknown name are the default."""
    if setting in QUALITY_PRESETS:
        return setting
    if setting and setting != "qvbr":
        log(f"unknown quality {setting!r}, using {DEFAULT_QUALITY}")
    return DEFAULT_QUALITY


def _quality_factor(settings):
    q_av1, q_h26x, target, ceiling = QUALITY_PRESETS[quality_preset(settings.get("quality"))]
    q = q_av1 if str(settings.get("codec", "av1_10bit")).startswith("av1") else q_h26x
    return q, target, ceiling


def ffmpeg_video_opts(settings):
    """The -ffmpeg-video-opts string: the raw override, a raw legacy `quality`, else the preset's QVBR."""
    raw = settings.get("ffmpeg_video_opts") or ""
    quality = str(settings.get("quality") or "")
    if not raw and "=" in quality:
        raw = quality
    if raw:
        return raw
    q, target, ceiling = _quality_factor(settings)
    # Merged last, right before avcodec_open2, so rc_mode wins over -bm cbr and b over -q.
    return f"rc_mode=QVBR;global_quality={q};b={target * 1000};maxrate={ceiling * 1000};bufsize={ceiling * 2000}"


def container_ext(settings):
    ext = settings.get("container") or "mkv"
    if ext not in CONTAINERS:
        log(f"unknown container {ext!r}, using mkv")
        return "mkv"
    return ext


def size_limit(settings):
    """(W, H) the output must fit in, None for the source's own size."""
    raw = str(settings.get("size") or "native").strip().lower()
    m = re.fullmatch(r"(\d+)x(\d+)", raw)
    if not m or "0" in m.groups():
        if raw not in ("native", "0x0"):
            log(f"size {raw!r} is not WxH, recording at the source's size")
        return None
    return int(m.group(1)), int(m.group(2))


def audio_codec(settings):
    codec = settings.get("audio_codec") or "opus"
    if codec not in AUDIO_CODECS:
        log(f"unknown audio codec {codec!r}, using opus")
        return "opus"
    return codec


def audio_bitrate_kbps(settings):
    """The audio bitrate in kbps, None for the encoder's own default."""
    raw = settings.get("audio_bitrate", "auto")
    if raw in ("auto", "", None, 0, "0"):
        return None
    try:
        return int(raw)
    except (TypeError, ValueError):
        log(f"audio bitrate {raw!r} is not a number, using the encoder's default")
        return None


def gsr_extra_args(settings):
    raw = settings.get("gsr_extra_args") or ""
    try:
        return shlex.split(raw)
    except ValueError as e:
        log(f"gsr_extra_args unparsable ({e}), ignored")
        return []


def gsr_args(settings, screen, output_path, token_path=None):
    """The gpu-screen-recorder argv (no systemd-run wrapper): the screen, or the game's window through the
    GNOME picker with `token_path` remembering the pick for the game's next launches."""
    cursor = "yes" if settings.get("cursor") else "no"
    fps = str(resolve_fps(settings.get("fps", 60), screen))
    codec = settings.get("codec", "av1_10bit")
    size = size_limit(settings)
    bitrate = audio_bitrate_kbps(settings)
    ac = audio_codec(settings)
    if ac == "flac":
        # gpu-screen-recorder's man page: "FLAC temporarily disabled".
        log("flac is disabled in gpu-screen-recorder, using opus")
        ac = "opus"
    if token_path:
        target = ["-w", "portal", "-restore-portal-session", "yes", "-portal-session-token-filepath", token_path]
    else:
        target = ["-w", screen]
    return [
        "gpu-screen-recorder",
        *target,
        "-cursor", cursor,
        "-f", fps,
        "-fm", "vfr",
        "-c", container_ext(settings),
        *(["-s", f"{size[0]}x{size[1]}"] if size else []),
        "-k", codec,
        "-ac", ac,
        *(["-ab", str(bitrate)] if bitrate is not None else []),
        "-tune", "quality",
        # cbr is the base the QVBR override needs: gsr's vbr branch pins qmin = qmax.
        "-bm", "cbr",
        "-q", "20000",
        *audio_args(settings.get("audio")),
        "-ffmpeg-video-opts", ffmpeg_video_opts(settings),
        *gsr_extra_args(settings),
        "-o", output_path,
    ]


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


def output_path(pending_dir, session_id, settings):
    return os.path.join(pending_dir, f"{session_id}.{container_ext(settings)}")


def list_windows():
    """The shell's toplevels through the universe extension (org.universe.Windows.List); [] when it does not answer."""
    try:
        out = subprocess.run(
            ["busctl", "--user", "--json=short", "call", WINDOWS_BUS_NAME, "/org/universe/Windows", WINDOWS_BUS_NAME, "List"],
            capture_output=True, text=True, check=True, timeout=5,
        ).stdout
        return json.loads(json.loads(out)["data"][0])
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, IndexError, TypeError):
        return []


def pick_window(windows, unit_cg):
    """The largest visible toplevel of the game's cgroup, else None."""
    mine = [w for w in windows
            if not w.get("hidden") and not w.get("minimized") and w.get("pid") and pid_in_unit(w["pid"], unit_cg)]
    return max(mine, key=lambda w: w.get("width", 0) * w.get("height", 0), default=None)


def wait_for_window(unit_cg, wait_s, poll_s=0.5):
    deadline = time.monotonic() + wait_s
    while True:
        w = pick_window(list_windows(), unit_cg)
        if w or time.monotonic() >= deadline:
            return w
        time.sleep(poll_s)


def window_wait_s(settings):
    try:
        return max(0, int(settings.get("window_wait_s", SETTINGS_DEFAULTS["window_wait_s"])))
    except (TypeError, ValueError):
        log(f"window_wait_s {settings.get('window_wait_s')!r} is not a number, using {SETTINGS_DEFAULTS['window_wait_s']}")
        return SETTINGS_DEFAULTS["window_wait_s"]


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


def extension_ready():
    """Whether org.universe.Windows answers: one bus round-trip when it is up; the enable and its wait otherwise
    (EnableExtension runs enable() asynchronously in the shell, so the name can lag the call)."""
    if not is_gnome():
        return False
    if not extension_installed():
        log(f"{EXTENSION_UUID} not installed")
        return False
    if bus_name_has_owner(WINDOWS_BUS_NAME):
        return True
    enable_extension()
    deadline = time.monotonic() + 3
    while not bus_name_has_owner(WINDOWS_BUS_NAME):
        if time.monotonic() > deadline:
            log(f"{EXTENSION_UUID} installed but not loaded (log out once to load it)")
            return False
        time.sleep(0.25)
    return True


def show_osd(label, icon="video-display-symbolic"):
    """The shell's OSD through the universe extension; nothing when it is not loaded."""
    try:
        subprocess.run(
            ["busctl", "--user", "call", WINDOWS_BUS_NAME, "/org/universe/Windows", WINDOWS_BUS_NAME,
             "ShowOSD", "ssd", "--", icon, label, "-1"],
            capture_output=True, text=True, timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def companion_path(pending_dir, session_id):
    return os.path.join(pending_dir, f"{session_id}.json")


def write_companion(pending_dir, session_id, unit, path, mode):
    with open(companion_path(pending_dir, session_id), "w") as f:
        json.dump({
            "unit": f"{unit}.service",
            "output": path,
            "mode": mode,
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        }, f)


def bus_name_has_owner(name):
    r = subprocess.run(
        ["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus",
         "org.freedesktop.DBus", "NameHasOwner", "s", name],
        capture_output=True, text=True,
    )
    return r.returncode == 0 and r.stdout.strip() == "b true"


def universe_bin():
    return os.environ.get("UNIVERSE_BIN") or "universe"
