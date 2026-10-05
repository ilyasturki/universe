import importlib.machinery
import importlib.util
import json
import os
import shutil
import stat
import struct
import subprocess
import sys
import threading
import time
from datetime import datetime, timedelta
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
BIN_DIR = MODULE_DIR / "bin"

# Loaded under a unique name so it does not shadow other modules' bin/_common.py.
_spec = importlib.util.spec_from_file_location("capture_common", BIN_DIR / "_common.py")
_common = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_common)

QVBR_OPTS = "rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000"
SESSION_ID = "20260911-120000"
GAME_UNIT = "universe-game-x-1.service"


def _write_shim(path, body):
    path.write_text(f"#!{shutil.which('bash')}\n{body}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


@pytest.fixture
def fakebin(tmp_path):
    bindir = tmp_path / "fakebin"
    bindir.mkdir()
    logs = tmp_path / "calls"
    logs.mkdir()

    _write_shim(bindir / "systemd-run", f'printf "%s\\n" "$@" > "{logs}/systemd-run.args"\nexit 0\n')
    _write_shim(
        bindir / "systemctl",
        f'''printf "%s\\n" "$@" >> "{logs}/systemctl.args"
case "$*" in
  *FreezerState*) echo "${{FAKE_FREEZER_STATE:-running}}";;
  *is-active*) echo "${{FAKE_UNIT_STATE:-active}}";;
esac
exit "${{FAKE_KILL_EXIT:-0}}"''',
    )
    _write_shim(
        bindir / "universe",
        f'''printf "%s\\n" "$@" >> "{logs}/universe.args"
if [ "${{FAKE_UNIVERSE_EXIT:-0}}" != "0" ]; then echo "universe: unavailable: no shell" >&2; exit "${{FAKE_UNIVERSE_EXIT}}"; fi
case "$1" in
  screen-mode) hz="${{FAKE_REFRESH:-}}"; [ -n "$hz" ] || {{ [ "$2" = HDMI-A-1 ] && hz=60 || hz=120; }}
    echo "{{\\"screen\\":\\"$2\\",\\"width\\":3840,\\"height\\":2160,\\"refresh\\":$hz,\\"vrr\\":false}}"; exit 0;;
  session-window) echo "${{FAKE_WINDOW_JSON:-null}}"; exit 0;;
esac
echo "/mnt/recordings/games/fake/session.mkv"
exit 0''',
    )
    _write_shim(bindir / "ffprobe", 'echo "${FAKE_DURATION:-300}"\nexit 0\n')
    _write_shim(
        bindir / "gsr-cli",
        f'''printf "%s\\n" "$@" >> "{logs}/gsr-cli.args"
case "$3" in
  status) exit "${{FAKE_RECORDER_DOWN:-0}}";;
  set-paused) [ "${{FAKE_PAUSE_EXIT:-0}}" = 0 ] || {{ echo "error: no recording" >&2; exit "$FAKE_PAUSE_EXIT"; }}; exit 0;;
  stop) [ -z "${{FAKE_STOP_PATH:-}}" ] && {{ echo "error: not running" >&2; exit 1; }}; echo "$FAKE_STOP_PATH"; exit 0;;
esac
exit 0''',
    )
    _write_shim(bindir / "trash", f'printf "%s\\n" "$@" > "{logs}/trash.args"\nrm -f "$1"\nexit 0\n')
    _write_shim(
        bindir / "gpu-screen-recorder",
        f'''if [ "$1" = "--info" ]; then
  printf "section=gpu_info\\nvendor|amd\\nsection=video_codecs\\n%s\\nsection=capture_options\\nDP-1|3840x2160\\n" "${{FAKE_CODECS-h264
hevc
hevc_10bit
av1
av1_10bit}}"
  exit 0
fi
printf "%s\\n" "$@" >> "{logs}/gsr.args"
for ((i=1; i<=$#; i++)); do
  if [ "${{!i}}" = "-o" ]; then j=$((i+1)); echo fake > "${{!j}}"; [ -n "${{FAKE_FIRST_FRAME_US:-}}" ] && printf "monotonic_microsec realtime_microsec\\n1000 %s\\n" "$FAKE_FIRST_FRAME_US" > "${{!j}}.ts"; fi
done
exit 0''',
    )
    return {"bin": bindir, "logs": logs}


def env_for(tmp_path, fakebin, settings, extra=None):
    env = dict(os.environ)
    env["PATH"] = f"{fakebin['bin']}:{env.get('PATH', '')}"
    env["SESSION_ID"] = SESSION_ID
    env["SESSION_UNIT"] = GAME_UNIT
    env["MODULE_DATA_DIR"] = str(tmp_path / "data")
    env["JOURNAL_DIR"] = str(tmp_path / "journal")
    env["SCREENSHOTS_DIR"] = str(tmp_path / "screenshots")
    env["UNIVERSE_BIN"] = str(fakebin["bin"] / "universe")
    env["CAPTURE_RECORDER_WAIT_S"] = "0"
    env["MODULE_SETTINGS_JSON"] = json.dumps(settings)
    env.setdefault("SESSION_SCREEN", "DP-1")
    home = tmp_path / "home"
    home.mkdir(exist_ok=True)
    env["HOME"] = str(home)
    env["XDG_DATA_DIRS"] = str(tmp_path / "datadirs")
    env["XDG_CURRENT_DESKTOP"] = "GNOME"
    if extra:
        env.update(extra)
    return env


def universe_calls(fakebin):
    path = fakebin["logs"] / "universe.args"
    return path.read_text().splitlines() if path.exists() else []


def osd_icons(fakebin):
    calls = universe_calls(fakebin)
    return [calls[i + 2] for i, a in enumerate(calls) if a == "osd"]


def run(script, env):
    return subprocess.run([str(BIN_DIR / script)], env=env, capture_output=True, text=True, timeout=30, check=False)


def flag_values(args, flag):
    return [args[i + 1] for i, a in enumerate(args) if a == flag]


def pending_path(tmp_path):
    return str(tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv")


def gsr_case(id, settings, want, token=None, **env):
    return pytest.param(settings, want, token, env, id=id)


@pytest.mark.parametrize(
    ("settings", "want", "token", "env"),
    [
        gsr_case(
            "defaults",
            {},
            {
                "-w": ["DP-1"],
                "-cursor": ["no"],
                "-f": ["60"],
                "-c": ["mkv"],
                "-s": [],
                "-k": ["av1_10bit"],
                "-ffmpeg-video-opts": [QVBR_OPTS],
                "-a": ["default_output"],
                "-ac": ["opus"],
                "-ab": [],
                "-ipc": [_common.ipc_socket(SESSION_ID)],
                "-write-first-frame-ts": ["yes"],
            },
        ),
        gsr_case(
            "chosen",
            {
                "cursor": True,
                "codec": "hevc",
                "fps": 30,
                "quality": "high",
                "container": "mp4",
                "size": "2560x1440",
                "audio": "output+input",
                "audio_codec": "aac",
                "audio_bitrate": 160,
                "gsr_extra_args": "-cr full -keyint 2",
            },
            {
                "-cursor": ["yes"],
                "-k": ["hevc"],
                "-f": ["30"],
                "-ffmpeg-video-opts": ["rc_mode=QVBR;global_quality=27;b=10000000;maxrate=20000000;bufsize=40000000"],
                "-c": ["mp4"],
                "-s": ["2560x1440"],
                "-a": ["default_output", "default_input"],
                "-ac": ["aac"],
                "-ab": ["160"],
                "-cr": ["full"],
                "-keyint": ["2"],
            },
        ),
        gsr_case(
            "own encoder options, no flac",
            {"ffmpeg_video_opts": "rc_mode=CQP;qp=20", "audio": "none", "audio_codec": "flac"},
            {"-ffmpeg-video-opts": ["rc_mode=CQP;qp=20"], "-a": [], "-ac": ["opus"]},
        ),
        gsr_case(
            "codec auto, an older card",
            {"codec": "auto"},
            {"-k": ["hevc"], "-ffmpeg-video-opts": [QVBR_OPTS.replace("global_quality=95", "global_quality=22")]},
            FAKE_CODECS="h264\nhevc",
        ),
        gsr_case("codec auto, no codec it knows", {}, {"-k": ["h264"]}, FAKE_CODECS="vp8"),
        gsr_case("fps auto", {"fps": "auto"}, {"-f": ["120"]}),
        gsr_case("fps auto, no mode", {"fps": "auto"}, {"-f": ["60"]}, FAKE_REFRESH="0"),
        gsr_case("fps auto, no CLI", {"fps": "auto"}, {"-f": ["60"]}, FAKE_UNIVERSE_EXIT="1"),
        gsr_case(
            "window", {}, {"-w": ["portal"], "-restore-portal-session": ["yes"], "-portal-session-token-filepath": ["/t/dead-cells"]}, token="/t/dead-cells"
        ),
    ],
)
def test_gsr_args_carry_the_settings(fakebin, monkeypatch, settings, want, token, env):
    monkeypatch.setenv("PATH", f"{fakebin['bin']}:{os.environ['PATH']}")
    monkeypatch.setenv("UNIVERSE_BIN", str(fakebin["bin"] / "universe"))
    for k, v in env.items():
        monkeypatch.setenv(k, v)
    args = _common.gsr_args(settings, "DP-1", "/o.mkv", token, SESSION_ID)
    assert args[0] == "gpu-screen-recorder" and flag_values(args, "-o") == ["/o.mkv"]
    assert {flag: flag_values(args, flag) for flag in want} == want
    assert universe_calls(fakebin) in ([], ["screen-mode", "DP-1", "--json"]), "the screen's mode is all it asks the CLI"


def test_start_runs_the_recorder_under_record_in_a_unit_bound_to_the_game(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen", "codec": "hevc"}, extra={"GAME_ID": "sample"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert f"--unit=universe-capture-{SESSION_ID}" in args and "--collect" in args
    assert {"MemoryHigh=4G", f"BindsTo={GAME_UNIT}", f"After={GAME_UNIT}", "TimeoutStopSec=10"} <= set(flag_values(args, "-p"))
    record = args.index(str(BIN_DIR / "record"))
    assert args[record + 1 : record + 3] == ["--", "gpu-screen-recorder"]
    gsr = args[record + 2 :]
    assert (flag_values(gsr, "-w"), flag_values(gsr, "-k"), flag_values(gsr, "-o")) == (["DP-1"], ["hevc"], [pending_path(tmp_path)])
    setenv = {a.split("=", 2)[1]: a.split("=", 2)[2] for a in args[:record] if a.startswith("--setenv=")}
    assert setenv["SESSION_ID"] == SESSION_ID and setenv["MODULE_DATA_DIR"] == env["MODULE_DATA_DIR"] and setenv["GAME_ID"] == "sample"
    assert setenv["PATH"] == env["PATH"], "gsr-cli and ffprobe are on the hook's PATH, not the manager's"
    assert "TERM" not in setenv
    assert universe_calls(fakebin) == [], "a screen recording waits on no window"


@pytest.mark.parametrize(("distro", "fix"), [("arch", "install gpu-screen-recorder"), ("debian", "setcap")], ids=["arch", "elsewhere"])
def test_check_words_the_gsr_fix_for_the_distribution(tmp_path, distro, fix):
    _write_shim(tmp_path / "gpu-screen-recorder", "exit 0")
    env = {"PATH": str(tmp_path), "UNIVERSE_DISTRO": distro}
    result = subprocess.run([sys.executable, str(BIN_DIR / "check")], env=env, capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    lines = [json.loads(line) for line in result.stdout.splitlines()]
    assert [(c["check"], c["ok"]) for c in lines] == [("gpu-screen-recorder", True), ("gsr-cli", False), ("gsr-kms-server", False)]
    assert all(fix in c["fix"] and c["component"] == "gpu-screen-recorder" for c in lines)


def test_check_fails_a_gsr_kms_server_without_cap_sys_admin(tmp_path):
    for name in ("gpu-screen-recorder", "gsr-cli", "gsr-kms-server"):
        _write_shim(tmp_path / name, "exit 0")
    env = {"PATH": str(tmp_path), "UNIVERSE_DISTRO": "debian"}
    result = subprocess.run([sys.executable, str(BIN_DIR / "check")], env=env, capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    lines = [json.loads(line) for line in result.stdout.splitlines()]
    assert [(c["check"], c["ok"]) for c in lines] == [("gpu-screen-recorder", True), ("gsr-cli", True), ("gsr-kms-server", False)]
    assert "setcap cap_sys_admin+ep" in lines[2]["fix"]


def _vfs_cap(magic, permitted):
    return struct.pack("<IIIII", magic, permitted, 0, 0, 0)


@pytest.mark.parametrize(
    ("xattr", "ok"),
    [
        (_vfs_cap(0x02000001, 1 << 21), True),
        (_vfs_cap(0x03000001, 1 << 21 | 1 << 8) + struct.pack("<I", 0), True),
        (_vfs_cap(0x02000000, 1 << 21), False),
        (_vfs_cap(0x02000001, 1 << 24), False),
        (b"", False),
    ],
    ids=["v2", "v3-with-setpcap", "not-effective", "sys-resource-only", "empty"],
)
def test_check_reads_cap_sys_admin_from_the_file_capability(xattr, ok):
    loader = importlib.machinery.SourceFileLoader("capture_check", str(BIN_DIR / "check"))
    check = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
    loader.exec_module(check)
    assert check.sys_admin(xattr) is ok


def test_fps_choices_stop_at_the_screens_refresh_rate(tmp_path, fakebin):
    def choices(**extra):
        env = env_for(tmp_path, fakebin, {}, extra=extra)
        if "SESSION_SCREEN" not in extra:
            del env["SESSION_SCREEN"]
        result = subprocess.run([str(BIN_DIR / "choices"), "fps"], env=env, capture_output=True, text=True, timeout=30, check=False)
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)

    assert choices(SESSION_SCREEN="HDMI-A-1") == ["auto", "60", "30"]
    assert choices(SESSION_SCREEN="DP-1") == ["auto", "120", "90", "60", "30"]
    assert choices(FAKE_REFRESH="0") == ["auto", "120", "90", "60", "30"], "no mode: every rate stays"
    # The settings form runs choices without a session: the profile's default screen.
    assert (fakebin["logs"] / "universe.args").read_text().endswith("screen-mode\n--json\n")


def test_start_enabled_false_exits_early(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"enabled": False})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "systemd-run.args").exists()


def test_start_reports_a_recorder_gone_at_once(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"FAKE_RECORDER_DOWN": "1", "FAKE_UNIT_STATE": "failed"})
    result = run("start", env)
    assert result.returncode == 1, result.stderr
    assert osd_icons(fakebin) == ["dialog-warning-symbolic"]


def test_start_opens_the_timeline_before_the_recorder(tmp_path, fakebin):
    """A recorder that outlives its silent socket still has the timeline the supervisor reads."""
    env = env_for(tmp_path, fakebin, {}, extra={"FAKE_RECORDER_DOWN": "1"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "socket did not come up" in result.stderr
    assert _timeline(tmp_path) is not None


def test_pre_asks_gamescope_to_composite_only_for_a_window_recording(tmp_path, fakebin):
    env_file = tmp_path / "env"
    for settings, want in (
        ({"source": "window"}, "UNIVERSE_GAMESCOPE_ARGS=--force-composition\n"),
        ({"source": "screen"}, ""),
        ({"source": "window", "enabled": False}, ""),
    ):
        env_file.write_text("")
        result = run("pre", env_for(tmp_path, fakebin, settings, {"UNIVERSE_ENV_FILE": str(env_file)}))
        assert result.returncode == 0, result.stderr
        assert env_file.read_text() == want, settings


def _seed_pending(tmp_path):
    pending = tmp_path / "data" / "pending"
    pending.mkdir(parents=True)
    mkv = pending / f"{SESSION_ID}.mkv"
    mkv.write_bytes(b"fake mkv contents")
    return mkv


def test_stop_trashes_a_short_recording_and_its_timeline(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "5"})
    assert run("start", env).returncode == 0
    mkv = Path(pending_path(tmp_path))
    mkv.write_bytes(b"x")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "trash.args").read_text().splitlines() == [str(mkv)]
    assert not mkv.exists() and _timeline(tmp_path) is None
    assert universe_calls(fakebin) == []


def test_stop_cli_failure_leaves_file_and_exits_nonzero(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999", "FAKE_UNIVERSE_EXIT": "1"})
    result = run("stop", env)
    assert result.returncode == 1
    assert "left in pending" in result.stderr
    assert mkv.exists()


def test_stop_no_recording_is_a_noop(tmp_path, fakebin):
    result = run("stop", env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "universe.args").exists()


WINDOW_JSON = (
    '{"id":"7","pid":4242,"wm_class":"gamescope","title":"Dead Cells","focused":true,"x":0,"y":0,"width":3840,"height":2160,"hidden":false,"minimized":false}'
)


def test_start_window_records_through_the_portal_once_the_window_is_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 45}, extra={"FAKE_WINDOW_JSON": WINDOW_JSON, "GAME_ID": "dead-cells"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert universe_calls(fakebin) == ["session-window", "--wait", "45", "--json"], "the window came: no OSD"
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["portal"]
    assert flag_values(args, "-portal-session-token-filepath") == [str(tmp_path / "data" / "portal" / "dead-cells")]
    assert (tmp_path / "data" / "portal").is_dir()


@pytest.mark.parametrize("answer", [{"FAKE_WINDOW_JSON": "null"}, {"FAKE_UNIVERSE_EXIT": "1"}], ids=["no window in time", "the CLI fails"])
def test_start_window_records_the_screen_when_no_window_comes(tmp_path, fakebin, answer):
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 0}, extra={"GAME_ID": "dead-cells", **answer})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert flag_values((fakebin["logs"] / "systemd-run.args").read_text().splitlines(), "-w") == ["DP-1"]
    assert osd_icons(fakebin) == ["video-display-symbolic"], "the player is told"


def test_show_osd_goes_through_the_core_past_a_dash_led_label(tmp_path, fakebin):
    env = dict(os.environ, PATH=f"{fakebin['bin']}:{os.environ['PATH']}")
    env.pop("UNIVERSE_BIN", None)
    result = subprocess.run(
        [sys.executable, "-c", "import _common; _common.show_osd('-1 frame')"], cwd=BIN_DIR, env=env, capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "universe.args").read_text() == "osd\n--\nvideo-display-symbolic\n-1 frame\n"


def _timeline(tmp_path):
    path = tmp_path / "data" / "pending" / f"{SESSION_ID}.timeline.json"
    return json.loads(path.read_text()) if path.exists() else None


def _pauses(fakebin):
    """The `set-paused` values sent to the recorder, in order."""
    lines = (fakebin["logs"] / "gsr-cli.args").read_text().splitlines() if (fakebin["logs"] / "gsr-cli.args").exists() else []
    return [lines[i + 1] for i, a in enumerate(lines) if a == "set-paused"]


def test_freeze_before_start_is_a_noop_and_start_catches_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("freeze", env).returncode == 0
    assert _timeline(tmp_path) is None and _pauses(fakebin) == []

    result = run("start", dict(env, FAKE_FREEZER_STATE="frozen"))
    assert result.returncode == 0, result.stderr
    state = _timeline(tmp_path)
    assert state["paused"] and len(state["pauses"]) == 1 and state["pauses"][0][1] is None
    assert state["started_at"] <= state["pauses"][0][0]
    assert _pauses(fakebin) == ["true"]
    args = (fakebin["logs"] / "gsr-cli.args").read_text().splitlines()
    assert args[:2] == ["-ipc", _common.ipc_socket(SESSION_ID)] and args[2] == "status", "the socket is waited for before the catch-up"


def test_freeze_and_thaw_toggle_the_recorder_once_each(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("start", env).returncode == 0
    assert _timeline(tmp_path) == {"started_at": _timeline(tmp_path)["started_at"], "paused": False, "pauses": []}
    assert _pauses(fakebin) == []

    assert run("freeze", env).returncode == 0
    assert run("freeze", env).returncode == 0
    assert _pauses(fakebin) == ["true"] and _timeline(tmp_path)["paused"]

    assert run("thaw", env).returncode == 0
    assert run("thaw", env).returncode == 0
    state = _timeline(tmp_path)
    assert _pauses(fakebin) == ["true", "false"] and not state["paused"]
    assert len(state["pauses"]) == 1 and state["pauses"][0][0] <= state["pauses"][0][1]


def test_freeze_keeps_its_state_when_the_recorder_refuses(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("start", env).returncode == 0
    result = run("freeze", dict(env, FAKE_PAUSE_EXIT="1"))
    assert result.returncode == 0 and "pause failed" in result.stderr
    assert _timeline(tmp_path) == {"started_at": _timeline(tmp_path)["started_at"], "paused": False, "pauses": []}


def test_stop_closes_an_open_pause_and_hands_the_timeline_over(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    assert run("freeze", env).returncode == 0
    mkv = tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv"
    mkv.write_bytes(b"x")
    timeline = tmp_path / "data" / "pending" / f"{SESSION_ID}.timeline.json"

    _write_shim(fakebin["bin"] / "universe", f'cp "$5" "{tmp_path}/handed.json"\necho filed\nexit 0')
    result = run("stop", dict(env, FAKE_STOP_PATH=str(mkv)))
    assert result.returncode == 0, result.stderr
    handed = json.loads((tmp_path / "handed.json").read_text())
    assert not handed["paused"] and len(handed["pauses"]) == 1 and handed["pauses"][0][1] is not None
    assert not timeline.exists() and not (tmp_path / "data" / "pending" / f"{SESSION_ID}.timeline.json.lock").exists()
    args = (fakebin["logs"] / "gsr-cli.args").read_text().splitlines()
    assert args[-3:] == ["-ipc", _common.ipc_socket(SESSION_ID), "stop"]
    assert not (fakebin["logs"] / "systemctl.args").exists() or "stop" not in (fakebin["logs"] / "systemctl.args").read_text(), "the recorder saved on its own"


def test_stop_falls_back_to_the_unit_when_the_socket_is_gone(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"container": "mp4", "min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    mp4 = tmp_path / "data" / "pending" / f"{SESSION_ID}.mp4"
    assert flag_values((fakebin["logs"] / "systemd-run.args").read_text().splitlines(), "-o") == [str(mp4)]
    mp4.write_bytes(b"x")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "systemctl.args").read_text().splitlines()[-3:] == ["--user", "stop", f"universe-capture-{SESSION_ID}.service"]
    assert universe_calls(fakebin)[:3] == ["recording-file", SESSION_ID, str(mp4)], "the unit's file, whichever container"


def test_stop_takes_started_at_from_the_first_frame(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    assert run("freeze", env).returncode == 0
    assert run("thaw", env).returncode == 0
    before = _timeline(tmp_path)
    # The recorder's first frame lands an hour after the unit started (the picker sat open), after the pause the timeline saw
    first_frame = datetime.fromisoformat(before["started_at"]) + timedelta(hours=1)
    mkv = tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv"
    mkv.write_bytes(b"x")
    (tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv.ts").write_text(
        f"monotonic_microsec realtime_microsec\n1000 {int(first_frame.timestamp() * 1_000_000)}\n"
    )
    _write_shim(fakebin["bin"] / "universe", f'cp "$5" "{tmp_path}/handed.json"\necho filed\nexit 0')
    assert run("stop", dict(env, FAKE_STOP_PATH=str(mkv))).returncode == 0
    handed = json.loads((tmp_path / "handed.json").read_text())
    assert datetime.fromisoformat(handed["started_at"]) == first_frame.replace(microsecond=0)
    assert handed["pauses"] == [], "a pause closed before the first frame is not on the file"
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv.ts").exists()


def test_stop_discards_an_unreadable_recording(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    _write_shim(fakebin["bin"] / "ffprobe", "exit 1\n")
    result = run("stop", env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_STOP_PATH": str(mkv)}))
    assert result.returncode == 0, result.stderr
    assert "unreadable recording" in result.stderr
    assert (fakebin["logs"] / "trash.args").read_text().splitlines() == [str(mkv)]
    assert not (fakebin["logs"] / "universe.args").exists()


def test_size_limit_and_audio_bitrate_parse():
    assert _common.size_limit({"size": "1920x1080"}) == (1920, 1080)
    for raw in ("native", "", "0x0", "wide", None):
        assert _common.size_limit({"size": raw}) is None
    assert _common.audio_bitrate_kbps({"audio_bitrate": "128"}) == 128
    for raw in ("auto", "", 0, "0"):
        assert _common.audio_bitrate_kbps({"audio_bitrate": raw}) is None
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'a b'"}) == ["-x", "a b"]
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'unterminated"}) == []


def test_record_hands_a_portal_recording_to_gsr_as_is(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    result = subprocess.run(
        [str(BIN_DIR / "record"), "--", "gpu-screen-recorder", "-w", "portal", "-o", str(tmp_path / "out.mkv")],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "gsr.args").read_text().splitlines() == ["-w", "portal", "-o", str(tmp_path / "out.mkv")]
    assert not (tmp_path / "data").exists(), "no supervision, no timeline"


# _record imports `_common` by that name: this module's, registered only while it loads, so other modules' tests keep theirs.
sys.modules["_common"] = _common
try:
    _spec_record = importlib.util.spec_from_file_location("capture_record", BIN_DIR / "_record.py")
    _record = importlib.util.module_from_spec(_spec_record)
    _spec_record.loader.exec_module(_record)
finally:
    del sys.modules["_common"]


@pytest.fixture
def livebin(fakebin):
    """A recorder that runs until `gsr-cli stop`, as the real one does, plus the DRM tree the supervisor polls."""
    logs = fakebin["logs"]
    _write_shim(
        fakebin["bin"] / "gpu-screen-recorder",
        f'''printf "%s\\n" "$@" >> "{logs}/gsr.args"
out=; for ((i=1; i<=$#; i++)); do [ "${{!i}}" = -o ] && {{ j=$((i+1)); out="${{!j}}"; }}; done
if [ -e "{logs}/gsr.fail" ]; then echo "monitor not found" >&2; exit 3; fi
echo fake > "$out"
printf "monotonic_microsec realtime_microsec\\n1000 %s\\n" "$(( $(date +%s) * 1000000 ))" > "$out.ts"
echo "$out" > "{logs}/gsr.current"
trap 'exit 0' TERM
while [ ! -e "$out.stopflag" ]; do sleep 0.05; done
rm -f "$out.stopflag"
exit 0''',
    )
    _write_shim(
        fakebin["bin"] / "gsr-cli",
        f'''printf "%s\\n" "$@" >> "{logs}/gsr-cli.args"
case "$3" in
  status) [ -e "{logs}/gsr.current" ] || exit 1; exit 0;;
  set-paused) exit 0;;
  stop) [ -e "{logs}/gsr.current" ] || {{ echo "error: not running" >&2; exit 1; }}; out=$(cat "{logs}/gsr.current"); rm -f "{logs}/gsr.current"; touch "$out.stopflag"; echo "$out"; exit 0;;
esac
exit 0''',
    )
    _write_shim(fakebin["bin"] / "ffprobe", 'case "$*" in *width*) echo "3840,2160";; *) echo "${FAKE_DURATION:-300}";; esac\nexit 0\n')
    drm = fakebin["bin"].parent / "drm"
    for name in ("card1-DP-1", "card1-HDMI-A-1", "card1", "renderD128"):
        (drm / name).mkdir(parents=True)
    (drm / "card1-DP-1" / "status").write_text("connected\n")
    (drm / "card1-HDMI-A-1" / "status").write_text("disconnected\n")
    return {**fakebin, "drm": drm}


def _plug(livebin, **status):
    for name, s in status.items():
        (livebin["drm"] / f"card1-{name.replace('_', '-')}" / "status").write_text(f"{s}\n")


def _light(livebin, **enabled):
    for name, e in enabled.items():
        (livebin["drm"] / f"card1-{name.replace('_', '-')}" / "enabled").write_text(f"{e}\n")


def _wait_for(pred, timeout_s=10):
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if pred():
            return True
        time.sleep(0.05)
    return False


def _supervise(tmp_path, livebin, monkeypatch, up=True, **extra):
    """The supervisor in a thread on the session's open timeline, as bin/start leaves it; `up` waits for its first recorder."""
    env = env_for(tmp_path, livebin, {}, extra=extra)
    # The supervisor runs in-process here: its children read the real environment.
    for k, v in env.items():
        monkeypatch.setenv(k, v)
    output = pending_path(tmp_path)
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True):
        pass
    rec = _record.Recorder(
        ["gpu-screen-recorder", "-w", "DP-1", "-f", "60", "-o", output], SESSION_ID, env["MODULE_DATA_DIR"], GAME_UNIT, drm_dir=str(livebin["drm"])
    )
    rec.poll_s = 0.05
    done = []
    thread = threading.Thread(target=lambda: done.append(rec.run()), daemon=True)
    thread.start()
    assert not up or _wait_for((livebin["logs"] / "gsr.current").exists)
    return env, thread, done


def _gsr_runs(livebin):
    text = (livebin["logs"] / "gsr.args").read_text() if (livebin["logs"] / "gsr.args").exists() else ""
    runs, cur = [], []
    for line in text.splitlines():
        cur.append(line)
        if len(cur) >= 2 and cur[-2] == "-o":
            runs.append(cur)
            cur = []
    return runs


def _end_session(env, thread):
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID) as state:
        state["stopping"] = True
    stop = subprocess.run(["gsr-cli", "-ipc", "x", "stop"], env=env, capture_output=True, text=True, check=False)
    thread.join(timeout=10)
    return stop.stdout.strip()


def test_record_follows_the_monitor_that_replaces_the_recorded_one(tmp_path, livebin, monkeypatch):
    env, thread, done = _supervise(tmp_path, livebin, monkeypatch)
    _plug(livebin, DP_1="disconnected")
    part1 = tmp_path / "data" / "pending" / f"{SESSION_ID}.part1.mkv"
    assert _wait_for(lambda: part1.exists() and not (livebin["logs"] / "gsr.current").exists())
    assert part1.with_name(part1.name + ".ts").exists(), "the sidecar moves with the part"
    state = _timeline(tmp_path)
    assert state["parts"] == [str(part1)] and len(state["pauses"]) == 1 and state["pauses"][0][1] is None, (
        "the gap is a pause until the next monitor's first frame"
    )
    assert len(_gsr_runs(livebin)) == 1, "no monitor yet: nothing to record"

    _plug(livebin, HDMI_A_1="connected")
    assert _wait_for(lambda: len(_gsr_runs(livebin)) == 2 and _timeline(tmp_path)["pauses"][0][1] is not None)
    second = _gsr_runs(livebin)[1]
    assert flag_values(second, "-w") == ["HDMI-A-1"] and flag_values(second, "-s") == ["3840x2160"] and flag_values(second, "-f") == ["60"]
    assert flag_values(second, "-o") == [pending_path(tmp_path)]
    state = _timeline(tmp_path)
    assert state["screen"] == "HDMI-A-1" and not state["paused"]
    calls = universe_calls(livebin)
    assert sum(a == "osd" and "HDMI-A-1" in calls[i + 3] for i, a in enumerate(calls)) == 1, "the player is told the new monitor once"

    assert _end_session(env, thread) == pending_path(tmp_path)
    assert done == [0] and len(_gsr_runs(livebin)) == 2, "the session's own stop is not a switch"


def test_record_follows_a_screen_the_desktop_stops_drawing_on(tmp_path, livebin, monkeypatch):
    """Both cables stay in: only `enabled` says which screen the desktop moved to."""
    _plug(livebin, HDMI_A_1="connected")
    _light(livebin, DP_1="enabled", HDMI_A_1="disabled")
    env, thread, done = _supervise(tmp_path, livebin, monkeypatch)
    _light(livebin, DP_1="disabled", HDMI_A_1="enabled")
    assert _wait_for(lambda: _timeline(tmp_path).get("screen") == "HDMI-A-1")
    assert flag_values(_gsr_runs(livebin)[1], "-w") == ["HDMI-A-1"]

    assert _end_session(env, thread) == pending_path(tmp_path)
    assert done == [0]


def test_record_retries_while_the_new_monitor_settles(tmp_path, livebin, monkeypatch):
    monkeypatch.setattr(_record, "RESTART_WAIT_S", 0.05)
    env, thread, done = _supervise(tmp_path, livebin, monkeypatch)
    (livebin["logs"] / "gsr.fail").touch()
    _plug(livebin, DP_1="disconnected", HDMI_A_1="connected")
    assert _wait_for(lambda: len(_gsr_runs(livebin)) >= 3)
    (livebin["logs"] / "gsr.fail").unlink()
    assert _wait_for(lambda: (livebin["logs"] / "gsr.current").exists() and _timeline(tmp_path)["pauses"][0][1] is not None)
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.part2.mkv").exists(), "a failed attempt leaves no part"
    assert all(flag_values(r, "-w") == ["HDMI-A-1"] for r in _gsr_runs(livebin)[1:])

    _end_session(env, thread)
    assert done == [0]


def test_record_gives_up_when_the_new_monitor_never_takes(tmp_path, livebin, monkeypatch):
    monkeypatch.setattr(_record, "RESTART_WAIT_S", 0.01)
    monkeypatch.setattr(_record, "RESTART_TRIES", 2)
    _, thread, done = _supervise(tmp_path, livebin, monkeypatch)
    (livebin["logs"] / "gsr.fail").touch()
    _plug(livebin, DP_1="disconnected", HDMI_A_1="connected")
    thread.join(timeout=10)
    assert done == [3], "the recorder's own exit code, as when it dies on a live monitor"
    assert len(_gsr_runs(livebin)) == 4
    state = _timeline(tmp_path)
    assert state["parts"] == [str(tmp_path / "data" / "pending" / f"{SESSION_ID}.part1.mkv")] and state["pauses"][0][1] is None


def test_record_exits_with_a_recorder_that_dies_on_a_live_monitor(tmp_path, livebin, monkeypatch):
    _, thread, done = _supervise(tmp_path, livebin, monkeypatch)
    Path(pending_path(tmp_path) + ".stopflag").touch()
    thread.join(timeout=10)
    assert done == [0] and len(_gsr_runs(livebin)) == 1
    assert "parts" not in _timeline(tmp_path)


def test_record_reports_a_recorder_that_dies_at_once(tmp_path, livebin, monkeypatch):
    """bin/start opens the timeline first: without it the supervisor read the death as a session already over."""
    (livebin["logs"] / "gsr.fail").touch()
    _, thread, done = _supervise(tmp_path, livebin, monkeypatch, up=False)
    thread.join(timeout=10)
    assert done == [3]


def test_record_pauses_a_restarted_recorder_while_the_game_is_frozen(tmp_path, livebin, monkeypatch):
    env, thread, done = _supervise(tmp_path, livebin, monkeypatch, FAKE_FREEZER_STATE="frozen")
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID) as state:
        state["paused"] = True
        state["pauses"].append([_common.now_rfc3339(), None])
    _plug(livebin, DP_1="disconnected", HDMI_A_1="connected")
    assert _wait_for(lambda: len(_gsr_runs(livebin)) == 2 and _timeline(tmp_path).get("screen") == "HDMI-A-1")
    state = _timeline(tmp_path)
    assert state["paused"] and len(state["pauses"]) == 1 and state["pauses"][0][1] is None, "the freeze's pause runs on"
    assert _pauses(livebin) == ["true"], "the new recorder is told to pause, whatever the timeline already says"
    _end_session(env, thread)
    assert done == [0]


def _seed_parts(tmp_path, first_frame_us=None):
    pending = tmp_path / "data" / "pending"
    pending.mkdir(parents=True, exist_ok=True)
    part1 = pending / f"{SESSION_ID}.part1.mkv"
    part1.write_bytes(b"part one")
    if first_frame_us:
        (pending / f"{SESSION_ID}.part1.mkv.ts").write_text(f"monotonic_microsec realtime_microsec\n1000 {first_frame_us}\n")
    current = pending / f"{SESSION_ID}.mkv"
    current.write_bytes(b"part two")
    with _common.timeline(str(tmp_path / "data"), SESSION_ID, create=True) as state:
        state["parts"] = [str(part1)]
    return part1, current


def test_stop_hands_a_recording_in_parts_to_a_finish_unit(tmp_path, fakebin):
    part1, current = _seed_parts(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_STOP_PATH": str(current)})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert args[:5] == ["--user", f"--unit=universe-capture-finish-{SESSION_ID}", "--collect", "--wait", "--quiet"]
    assert f"--setenv=PATH={env['PATH']}" in args and f"--setenv=MODULE_SETTINGS_JSON={env['MODULE_SETTINGS_JSON']}" in args
    assert args[-2:] == [str(BIN_DIR / "finish"), str(current)]
    assert _timeline(tmp_path)["stopping"] is True
    assert not (fakebin["logs"] / "universe.args").exists(), "the unit files it"
    assert part1.exists() and current.exists()


def test_finish_stitches_the_parts_and_files_one_recording(tmp_path, fakebin):
    first_frame = datetime.now().astimezone().replace(microsecond=0) - timedelta(hours=1)
    part1, current = _seed_parts(tmp_path, int(first_frame.timestamp() * 1_000_000))
    (tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv.ts").write_text("monotonic_microsec realtime_microsec\n1000 1\n")
    _write_shim(
        fakebin["bin"] / "ffmpeg",
        f'''printf "%s\\n" "$@" >> "{fakebin["logs"]}/ffmpeg.args"
for ((i=1; i<=$#; i++)); do [ "${{!i}}" = -i ] && {{ j=$((i+1)); cp "${{!j}}" "{fakebin["logs"]}/concat.list"; }}; done
echo stitched > "${{@: -1}}"
exit 0''',
    )
    _write_shim(fakebin["bin"] / "universe", f'cp "$5" "{tmp_path}/handed.json"\necho filed\nexit 0')
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    result = subprocess.run([str(BIN_DIR / "finish"), str(current)], env=env, capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    ff = (fakebin["logs"] / "ffmpeg.args").read_text().splitlines()
    assert ff[:8] == ["-v", "error", "-y", "-f", "concat", "-safe", "0", "-i"] and ff[-3:] == ["-c", "copy", str(current.with_name(f"{SESSION_ID}.stitch.mkv"))]
    assert (fakebin["logs"] / "concat.list").read_text() == f"file '{part1}'\nfile '{current}'\n"
    assert current.read_text() == "stitched\n" and not part1.exists()
    assert not list((tmp_path / "data" / "pending").glob("*.ts")) and not list((tmp_path / "data" / "pending").glob("*.parts"))
    handed = json.loads((tmp_path / "handed.json").read_text())
    assert datetime.fromisoformat(handed["started_at"]) == first_frame, "the recording starts with its first part"
    assert "filed" in result.stderr


def test_finish_files_the_longest_part_when_the_stitch_fails(tmp_path, fakebin):
    part1, current = _seed_parts(tmp_path)
    _write_shim(fakebin["bin"] / "ffmpeg", 'echo "concat: broken" >&2\nexit 1\n')
    _write_shim(fakebin["bin"] / "ffprobe", f'case "$*" in *"{part1}"*) echo 900;; *) echo 300;; esac\nexit 0\n')
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240})
    result = subprocess.run([str(BIN_DIR / "finish"), str(current)], env=env, capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    assert "ffmpeg concat failed" in result.stderr
    assert (fakebin["logs"] / "universe.args").read_text().splitlines() == [
        "recording-file",
        SESSION_ID,
        str(part1),
        "--timeline",
        str(tmp_path / "data" / "pending" / f"{SESSION_ID}.timeline.json"),
    ]
    assert part1.exists() and current.exists() and not (tmp_path / "data" / "pending" / f"{SESSION_ID}.stitch.mkv").exists()


def test_finish_files_the_part_left_when_the_session_ended_between_monitors(tmp_path, fakebin):
    part1, current = _seed_parts(tmp_path)
    current.unlink()
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    result = subprocess.run([str(BIN_DIR / "finish")], env=env, capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "universe.args").read_text().splitlines()[:3] == ["recording-file", SESSION_ID, str(part1)]


def test_active_outputs_and_argv_edits(tmp_path):
    drm = tmp_path / "drm"
    for name, status in (("card1-DP-1", "connected"), ("card1-HDMI-A-1", "disconnected"), ("card0-DP-3", "connected")):
        (drm / name).mkdir(parents=True)
        (drm / name / "status").write_text(status + "\n")
    (drm / "card1").mkdir()
    assert _common.active_outputs(str(drm)) == ["DP-1", "DP-3"], "no `enabled` file: every cabled connector counts"
    assert _common.active_outputs(str(tmp_path / "nope")) == []
    (drm / "card1-DP-1" / "enabled").write_text("disabled\n")
    (drm / "card0-DP-3" / "enabled").write_text("enabled\n")
    assert _common.active_outputs(str(drm)) == ["DP-3"], "a cabled connector nothing is drawn on is not one"
    (drm / "card0-DP-3" / "enabled").write_text("disabled\n")
    assert _common.active_outputs(str(drm)) == ["DP-1", "DP-3"], "nothing lit: the cabled ones stand"
    argv = ["gpu-screen-recorder", "-w", "DP-1", "-o", "out.mkv"]
    assert _common.with_flag(argv, "-w", "HDMI-A-1") == ["gpu-screen-recorder", "-w", "HDMI-A-1", "-o", "out.mkv"]
    assert _common.with_flag(argv, "-s", "1920x1080") == ["gpu-screen-recorder", "-w", "DP-1", "-s", "1920x1080", "-o", "out.mkv"]
    assert _common.flag_value(argv, "-o") == "out.mkv" and _common.flag_value(argv, "-s") is None
    assert _common.part_path("/p/20260911-120000.mkv", 2) == "/p/20260911-120000.part2.mkv"
