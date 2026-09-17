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


def ffmpeg_video_opts(settings):
    raw = settings.get("ffmpeg_video_opts") or ""
    if raw:
        return raw
    q_av1, q_h26x, target, ceiling = QUALITY_PRESETS.get(settings.get("quality"), QUALITY_PRESETS["very_high"])
    q = q_av1 if str(settings.get("codec", "av1_10bit")).startswith("av1") else q_h26x
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


def gsr_args(settings, screen, output_path, token_path=None):
    size = size_limit(settings)
    bitrate = audio_bitrate_kbps(settings)
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
        "-k", settings.get("codec", "av1_10bit"),
        "-ac", ac,
        *(["-ab", str(bitrate)] if bitrate is not None else []),
        "-tune", "quality",
        # cbr is the base the QVBR override needs: gsr's vbr branch pins qmin = qmax.
        "-bm", "cbr",
        "-q", "20000",
        *AUDIO_ARGS.get(settings.get("audio"), AUDIO_ARGS["output+input"]),
        "-ffmpeg-video-opts", ffmpeg_video_opts(settings),
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
    enabled = subprocess.run(
        ["busctl", "--user", "call", "org.gnome.Shell.Extensions",
         "/org/gnome/Shell/Extensions", "org.gnome.Shell.Extensions", "EnableExtension", "s", EXTENSION_UUID],
        capture_output=True, text=True,
    ).stdout.strip() == "b true"
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


def bus_name_has_owner(name):
    r = subprocess.run(
        ["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus",
         "org.freedesktop.DBus", "NameHasOwner", "s", name],
        capture_output=True, text=True,
    )
    return r.returncode == 0 and r.stdout.strip() == "b true"


def capture_unit(session_id):
    return f"universe-capture-{session_id}.service"


def timeline_path(data_dir, session_id):
    return os.path.join(data_dir, "pending", f"{session_id}.timeline.json")


def now_rfc3339():
    return datetime.now().astimezone().isoformat(timespec="seconds")


@contextlib.contextmanager
def timeline(data_dir, session_id, create=False):
    """The recorder's clock, `{"started_at", "paused", "pauses": [[from, to]]}`, locked for the block; None when no recording runs."""
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
    # SIGUSR2 toggles gpu-screen-recorder's pause; main only, or gsr-kms-server in the same cgroup dies of it.
    r = subprocess.run(["systemctl", "--user", "kill", "--kill-whom=main", "--signal=SIGUSR2", capture_unit(session_id)], capture_output=True, text=True)
    if r.returncode != 0:
        log(f"{'pause' if on else 'resume'} failed: {r.stderr.strip()}")
        return
    if on:
        state["pauses"].append([now_rfc3339(), None])
    elif state["pauses"] and state["pauses"][-1][1] is None:
        state["pauses"][-1][1] = now_rfc3339()
    state["paused"] = on


def game_frozen(unit):
    r = subprocess.run(["systemctl", "--user", "show", "-p", "FreezerState", "--value", unit], capture_output=True, text=True)
    return r.stdout.strip() in ("frozen", "freezing")
