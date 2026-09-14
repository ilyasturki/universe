"""Shared helpers for capture module hooks. Not a hook itself."""
import glob
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
GST_MUXERS = {"mkv": "matroskamux", "mp4": "mp4mux"}
GST_AUDIO_ENCODERS = {"opus": "opusenc", "aac": "fdkaacenc", "flac": "flacenc"}

SETTINGS_DEFAULTS = {
    "enabled": True,
    "source": "window",
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
    "va_encoder_opts": "",
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
    if ext not in GST_MUXERS:
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
    if codec not in GST_AUDIO_ENCODERS:
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


def gsr_args(settings, screen, output_path):
    """The gpu-screen-recorder argv (no systemd-run wrapper) — the screen-capture path."""
    cursor = "yes" if settings.get("cursor") else "no"
    fps = str(resolve_fps(settings.get("fps", 60), screen))
    codec = settings.get("codec", "av1_10bit")
    size = size_limit(settings)
    bitrate = audio_bitrate_kbps(settings)
    ac = audio_codec(settings)
    if ac == "flac":
        # gpu-screen-recorder's man page: "FLAC temporarily disabled".
        log("flac is disabled in gpu-screen-recorder, using opus for the screen path")
        ac = "opus"
    return [
        "gpu-screen-recorder",
        "-w", screen,
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


def cursor_mode(settings):
    return CURSOR_EMBEDDED if settings.get("cursor") else CURSOR_HIDDEN


def va_encoder_props(settings):
    """The VA encoder's rate-control properties: the raw override, else the preset's QVBR."""
    raw = settings.get("va_encoder_opts") or ""
    if raw:
        return raw.split()
    q, target, ceiling = _quality_factor(settings)
    return ["rate-control=qvbr", f"qp={q}", f"bitrate={ceiling}", f"target-percentage={target * 100 // ceiling}"]


def gst_audio_encoder(settings):
    codec = audio_codec(settings)
    bitrate = audio_bitrate_kbps(settings)
    encoder = [GST_AUDIO_ENCODERS[codec]]
    if bitrate is not None and codec != "flac":
        encoder.append(f"bitrate={bitrate * 1000}")
    return encoder


def audio_gst_branches(settings):
    setting = settings.get("audio")
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
            *gst_audio_encoder(settings), "!", "queue", "!", "mux.",
        ]
    return branch


def gst_window_args(node_id, settings, fps, output_path):
    """gst-launch argv that encodes a PipeWire window node to the container. fps None keeps the source rate."""
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
    caps = f"video/x-raw(memory:VAMemory),format={fmt}"
    size = size_limit(settings)
    if size:
        # A range fits the frame inside WxH at its own aspect, never upscaled — gsr's -s.
        caps += f",width=[1,{size[0]}],height=[1,{size[1]}],pixel-aspect-ratio=1/1"
    args += [
        "vapostproc", "!", caps, "!",
        encoder, *va_encoder_props(settings), "!",
        parser, "!", "queue", "!",
        GST_MUXERS[container_ext(settings)], "name=mux", "!",
        "filesink", f"location={output_path}",
    ]
    args += audio_gst_branches(settings)
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


def output_path(pending_dir, session_id, settings):
    return os.path.join(pending_dir, f"{session_id}.{container_ext(settings)}")


def segment_path(pending_dir, session_id, index, settings):
    return os.path.join(pending_dir, f"{session_id}-{index}.{container_ext(settings)}")


def _segment_index(session_id, path):
    m = re.search(rf"{re.escape(session_id)}-(\d+)\.(mkv|mp4)$", os.path.basename(path))
    return int(m.group(1)) if m else -1


def segment_paths(pending_dir, session_id):
    found = glob.glob(os.path.join(pending_dir, f"{session_id}-*.m*"))
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


def set_companion_mode(pending_dir, session_id, mode, reason):
    """Corrects the mode start wrote when record-window ends up recording the screen."""
    path = companion_path(pending_dir, session_id)
    try:
        with open(path) as f:
            data = json.load(f)
    except (OSError, ValueError):
        return
    data["mode"] = mode
    data["fallback"] = reason
    with open(path, "w") as f:
        json.dump(data, f)


def bus_name_has_owner(name):
    r = subprocess.run(
        ["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus",
         "org.freedesktop.DBus", "NameHasOwner", "s", name],
        capture_output=True, text=True,
    )
    return r.returncode == 0 and r.stdout.strip() == "b true"


def universe_bin():
    return os.environ.get("UNIVERSE_BIN") or "universe"
