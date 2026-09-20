import contextlib
import fcntl
import json
import os
import re
import shlex
import subprocess
import sys
import time
from datetime import datetime

# (AV1 q 0-255, H.26x q 0-51, target kbps, ceiling kbps): QVBR up to the ceiling; very_high measured ~7-8 GB/h on AMD.
QUALITY_PRESETS = {
    "medium": (150, 32, 6000, 12000),
    "high": (120, 27, 10000, 20000),
    "very_high": (95, 22, 16000, 32000),
    "ultra": (70, 17, 24000, 48000),
}

EXTENSION_UUID = "universe@ilyasturki.github.io"
WINDOWS_BUS_NAME = "org.universe.Windows"

AUDIO_ARGS = {"output": ["-a", "default_output"], "none": [], "output+input": ["-a", "default_output", "-a", "default_input"]}

# `codec = auto`: the first the card encodes, best first.
CODEC_PREFERENCE = ["av1_10bit", "hevc_10bit", "hevc", "h264"]


def log(msg):
    print(f"[capture] {msg}", file=sys.stderr, flush=True)


def load_settings():
    return json.loads(os.environ.get("MODULE_SETTINGS_JSON") or "{}")


def cli_json(args, timeout=20):
    cmd = [os.environ.get("UNIVERSE_BIN") or "universe", *args, "--json"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.SubprocessError) as e:
        log(f"universe {' '.join(args)}: {e}")
        return None
    if r.returncode != 0:
        log(f"universe {' '.join(args)}: {r.stderr.strip()}")
        return None
    try:
        return json.loads(r.stdout)
    except ValueError:
        return None


def screen_refresh_hz(screen):
    mode = cli_json(["screen-mode", *([screen] if screen else [])]) or {}
    return int(mode.get("refresh") or 0) or None


def resolve_fps(setting, screen):
    if setting != "auto":
        return int(setting)
    hz = screen_refresh_hz(screen)
    if hz is None:
        log(f"fps auto: no refresh rate for {screen}, using 60")
        return 60
    return hz


def supported_codecs():
    """The video codecs gpu-screen-recorder can encode here: the `video_codecs` section of its `--info`."""
    try:
        r = subprocess.run(["gpu-screen-recorder", "--info"], capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.SubprocessError) as e:
        log(f"gpu-screen-recorder --info: {e}")
        return []
    section, out = "", []
    for line in r.stdout.splitlines():
        line = line.strip()
        if line.startswith("section="):
            section = line[len("section="):]
        elif section == "video_codecs" and line:
            out.append(line)
    return out


def resolve_codec(settings):
    codec = str(settings.get("codec") or "auto")
    if codec != "auto":
        return codec
    have = supported_codecs()
    pick = next((c for c in CODEC_PREFERENCE if c in have), None)
    if pick is None:
        log(f"codec auto: gpu-screen-recorder lists none of {', '.join(CODEC_PREFERENCE)}, trying h264")
        return "h264"
    return pick


def ffmpeg_video_opts(settings, codec):
    raw = settings.get("ffmpeg_video_opts") or ""
    if raw:
        return raw
    q_av1, q_h26x, target, ceiling = QUALITY_PRESETS.get(settings.get("quality"), QUALITY_PRESETS["very_high"])
    q = q_av1 if codec.startswith("av1") else q_h26x
    # Merged last, right before avcodec_open2, so rc_mode wins over -bm cbr and b over -q.
    return f"rc_mode=QVBR;global_quality={q};b={target * 1000};maxrate={ceiling * 1000};bufsize={ceiling * 2000}"


def size_limit(settings):
    m = re.fullmatch(r"(\d+)x(\d+)", str(settings.get("size") or "").strip().lower())
    return (int(m.group(1)), int(m.group(2))) if m and int(m.group(1)) and int(m.group(2)) else None


def audio_bitrate_kbps(settings):
    raw = settings.get("audio_bitrate", "auto")
    return None if raw in ("auto", "", None, 0, "0") else int(raw)


def gsr_extra_args(settings):
    raw = settings.get("gsr_extra_args") or ""
    try:
        return shlex.split(raw)
    except ValueError as e:
        log(f"gsr_extra_args unparsable ({e}), ignored")
        return []


def ipc_socket(session_id):
    """gpu-screen-recorder's command socket for the session; unix paths are short, so the runtime dir, not the data dir."""
    return os.path.join(os.environ.get("XDG_RUNTIME_DIR") or "/tmp", f"universe-capture-{session_id}.sock")


def gsr_cli(session_id, *command, timeout=30):
    return subprocess.run(["gsr-cli", "-ipc", ipc_socket(session_id), *command], capture_output=True, text=True, timeout=timeout)


def wait_recorder(session_id, timeout_s=5, alive=None):
    """The socket comes up with the recorder, a moment after its unit; `alive` false cuts the wait short."""
    deadline = time.monotonic() + timeout_s
    while True:
        try:
            if gsr_cli(session_id, "status", timeout=5).returncode == 0:
                return True
        except (OSError, subprocess.SubprocessError):
            pass
        if time.monotonic() > deadline or (alive is not None and not alive()):
            return False
        time.sleep(0.25)


def gsr_args(settings, screen, output_path, token_path=None, session_id=None):
    size = size_limit(settings)
    bitrate = audio_bitrate_kbps(settings)
    codec = resolve_codec(settings)
    ac = settings.get("audio_codec") or "opus"
    if ac == "flac":
        # gpu-screen-recorder's man page: "FLAC temporarily disabled".
        log("flac is disabled in gpu-screen-recorder, using opus")
        ac = "opus"
    return [
        "gpu-screen-recorder",
        *(["-w", "portal", "-restore-portal-session", "yes", "-portal-session-token-filepath", token_path] if token_path else ["-w", screen]),
        "-cursor", "yes" if settings.get("cursor") else "no",
        "-f", str(resolve_fps(settings.get("fps", 60), screen)),
        "-fm", "vfr",
        "-c", settings.get("container") or "mkv",
        *(["-s", f"{size[0]}x{size[1]}"] if size else []),
        "-k", codec,
        "-ac", ac,
        *(["-ab", str(bitrate)] if bitrate is not None else []),
        "-tune", "quality",
        # cbr is the base the QVBR override needs: gsr's vbr branch pins qmin = qmax.
        "-bm", "cbr",
        "-q", "20000",
        *AUDIO_ARGS.get(settings.get("audio"), AUDIO_ARGS["output"]),
        "-ffmpeg-video-opts", ffmpeg_video_opts(settings, codec),
        *(["-ipc", ipc_socket(session_id)] if session_id else []),
        # The muxer's own first-frame instant, next to the file as <output>.ts
        "-write-first-frame-ts", "yes",
        *gsr_extra_args(settings),
        "-o", output_path,
    ]


def extension_ready():
    if "GNOME" not in (os.environ.get("XDG_CURRENT_DESKTOP") or ""):
        return False
    dirs = [os.path.expanduser("~/.local/share"), *(os.environ.get("XDG_DATA_DIRS") or "/usr/share").split(":")]
    if not any(os.path.isdir(os.path.join(d, "gnome-shell/extensions", EXTENSION_UUID)) for d in dirs):
        log(f"{EXTENSION_UUID} not installed")
        return False
    if bus_name_has_owner(WINDOWS_BUS_NAME):
        return True
    enabled = bus_call_bool(["org.gnome.Shell.Extensions", "/org/gnome/Shell/Extensions", "org.gnome.Shell.Extensions", "EnableExtension", "s", EXTENSION_UUID])
    if not enabled:
        log(f"{EXTENSION_UUID} installed but not loaded (log out once to load it)")
        return False
    # EnableExtension returns before enable() runs: the name lags the call.
    deadline = time.monotonic() + 3
    while not bus_name_has_owner(WINDOWS_BUS_NAME):
        if time.monotonic() > deadline:
            log(f"{EXTENSION_UUID} enabled but {WINDOWS_BUS_NAME} never came up")
            return False
        time.sleep(0.25)
    return True


def show_osd(label, icon="video-display-symbolic"):
    try:
        subprocess.run(
            ["busctl", "--user", "call", WINDOWS_BUS_NAME, "/org/universe/Windows", WINDOWS_BUS_NAME,
             "ShowOSD", "ssd", "--", icon, label, "-1"],
            capture_output=True, text=True, timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def bus_call_bool(call, timeout=None):
    """A session-bus call whose reply is one boolean; False on any failure."""
    try:
        r = subprocess.run(["busctl", "--user", "--json=short", "call", *call], capture_output=True, text=True, timeout=timeout)
        return r.returncode == 0 and json.loads(r.stdout)["data"] == [True]
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError):
        return False


def bus_name_has_owner(name):
    return bus_call_bool(["org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner", "s", name])


def timeline_path(data_dir, session_id):
    return os.path.join(data_dir, "pending", f"{session_id}.timeline.json")


def now_rfc3339():
    return datetime.now().astimezone().isoformat(timespec="seconds")


@contextlib.contextmanager
def timeline(data_dir, session_id, create=False):
    """Locked for the block; None when no recording runs."""
    path = timeline_path(data_dir, session_id)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + ".lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if create:
            state = {"started_at": now_rfc3339(), "paused": False, "pauses": []}
        else:
            try:
                with open(path) as f:
                    state = json.load(f)
            except (OSError, ValueError):
                yield None
                return
        yield state
        with open(path + ".tmp", "w") as f:
            json.dump(state, f)
        os.replace(path + ".tmp", path)


def drop_timeline(data_dir, session_id):
    path = timeline_path(data_dir, session_id)
    for p in (path, path + ".lock"):
        try:
            os.remove(p)
        except OSError:
            pass


def set_paused(state, session_id, on):
    if state["paused"] == on:
        return
    try:
        r = gsr_cli(session_id, "set-paused", "true" if on else "false")
    except (OSError, subprocess.SubprocessError) as e:
        log(f"{'pause' if on else 'resume'} failed: {e}")
        return
    if r.returncode != 0:
        log(f"{'pause' if on else 'resume'} failed: {(r.stderr or r.stdout).strip()}")
        return
    if on:
        state["pauses"].append([now_rfc3339(), None])
    elif state["pauses"] and state["pauses"][-1][1] is None:
        state["pauses"][-1][1] = now_rfc3339()
    state["paused"] = on


def game_frozen(unit):
    r = subprocess.run(["systemctl", "--user", "show", "-p", "FreezerState", "--value", unit], capture_output=True, text=True)
    return r.stdout.strip() in ("frozen", "freezing")


DRM_DIR = "/sys/class/drm"

# The env a transient unit gets from the hook: systemd-run starts from the manager's environment, not the caller's.
UNIT_ENV_PREFIXES = ("PATH", "HOME", "XDG_", "DBUS_", "UNIVERSE_", "MODULE_", "SESSION_", "GAME_")


def connected_outputs(drm_dir=None):
    """Connected connectors, sorted, named as desktop.rs names them: `card1-DP-1` is `DP-1`."""
    try:
        entries = os.listdir(drm_dir or DRM_DIR)
    except OSError:
        return []
    names = []
    for entry in entries:
        if "-" not in entry:
            continue
        try:
            with open(os.path.join(drm_dir or DRM_DIR, entry, "status")) as f:
                status = f.read().strip()
        except OSError:
            continue
        if status == "connected":
            names.append(entry.split("-", 1)[1])
    return sorted(names)


def unit_env_args():
    return [f"--setenv={k}={v}" for k, v in os.environ.items() if k.startswith(UNIT_ENV_PREFIXES)]


def flag_value(argv, flag):
    return next((argv[i + 1] for i, a in enumerate(argv[:-1]) if a == flag), None)


def with_flag(argv, flag, value):
    """argv with `flag value` set, a new pair ahead of `-o`; gpu-screen-recorder reads its pairs in any order."""
    out = list(argv)
    for i, a in enumerate(out[:-1]):
        if a == flag:
            out[i + 1] = value
            return out
    at = out.index("-o") if "-o" in out else len(out)
    return [*out[:at], flag, value, *out[at:]]


def part_path(output, n):
    """`<session>.part<n>.<ext>` beside `output`."""
    stem, ext = os.path.splitext(output)
    return f"{stem}.part{n}{ext}"


def probe_duration(path):
    try:
        result = subprocess.run(
            ["ffprobe", "-i", path, "-show_entries", "format=duration", "-v", "quiet", "-of", "csv=p=0"],
            capture_output=True, text=True, timeout=10, check=False,
        )
        return float(result.stdout.strip())
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def probe_size(path):
    """`(width, height)` of the first video stream; None when ffprobe cannot read it."""
    try:
        result = subprocess.run(
            ["ffprobe", "-i", path, "-select_streams", "v:0", "-show_entries", "stream=width,height", "-v", "quiet", "-of", "csv=p=0"],
            capture_output=True, text=True, timeout=10, check=False,
        )
        w, h = (int(v) for v in result.stdout.strip().split(",")[:2])
        return (w, h) if w > 0 and h > 0 else None
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def first_frame_at(path):
    """`-write-first-frame-ts`: `<monotonic_us> <realtime_us>` on the second line of <path>.ts; None without it."""
    try:
        with open(path + ".ts") as f:
            realtime_us = int(f.read().splitlines()[1].split()[1])
        return datetime.fromtimestamp(realtime_us / 1_000_000).astimezone().isoformat(timespec="seconds")
    except (OSError, ValueError, IndexError):
        return None


def remove_sidecar(path):
    with contextlib.suppress(OSError):
        os.remove(path + ".ts")
