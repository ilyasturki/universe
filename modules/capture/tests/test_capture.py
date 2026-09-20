import importlib.util
import json
import os
import shutil
import stat
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
esac
exit "${{FAKE_KILL_EXIT:-0}}"''',
    )
    _write_shim(
        bindir / "universe",
        f'''printf "%s\\n" "$@" >> "{logs}/universe.args"
if [ "${{FAKE_UNIVERSE_EXIT:-0}}" != "0" ]; then echo "universe: unavailable: no shell" >&2; exit "${{FAKE_UNIVERSE_EXIT}}"; fi
case "$1" in
  screen-mode) hz="${{FAKE_REFRESH:-}}"; [ -n "$hz" ] || {{ [ "$2" = HDMI-A-1 ] && hz=60 || hz=120; }}
    echo "{{\\"screen\\":\\"$2\\",\\"width\\":3840,\\"height\\":2160,\\"refresh\\":$hz}}"; exit 0;;
  session-window) echo "${{FAKE_WINDOW_JSON:-null}}"; exit 0;;
esac
echo "/mnt/recordings/games/fake/session.mkv"
exit 0''',
    )
    _write_shim(bindir / "ffprobe", 'echo "${FAKE_DURATION:-300}"\nexit 0\n')
    _write_shim(
        bindir / "busctl",
        f'''printf "%s\\n" "$@" >> "{logs}/busctl.args"
case "$*" in
  *NameHasOwner*) echo "{{\\"type\\":\\"b\\",\\"data\\":[${{FAKE_NAME_OWNED:-false}}]}}"; exit 0;;
  *EnableExtension*) echo "{{\\"type\\":\\"b\\",\\"data\\":[${{FAKE_NAME_OWNED:-false}}]}}"; exit 0;;
  *" Screenshot "*) [ "${{FAKE_SHOT_OK:-true}}" = true ] && echo fake > "$9"; echo "{{\\"type\\":\\"b\\",\\"data\\":[${{FAKE_SHOT_OK:-true}}]}}"; exit 0;;
esac
exit 0''',
    )
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


def install_fake_extension(env):
    uuid = "universe@ilyasturki.github.io"
    ext_dir = Path(env["HOME"]) / ".local/share/gnome-shell/extensions" / uuid
    ext_dir.mkdir(parents=True, exist_ok=True)
    (ext_dir / "metadata.json").write_text(json.dumps({"uuid": uuid}))


def run(script, env):
    return subprocess.run([str(BIN_DIR / script)], env=env, capture_output=True, text=True, timeout=30, check=False)


def flag_values(args, flag):
    return [args[i + 1] for i, a in enumerate(args) if a == flag]


def pending_path(tmp_path):
    return str(tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv")


def test_start_composes_gsr_command(tmp_path, fakebin):
    env = env_for(
        tmp_path,
        fakebin,
        {
            "enabled": True,
            "cursor": True,
            "codec": "hevc",
            "fps": 30,
            "audio": "output",
        },
    )
    result = run("start", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert f"--unit=universe-capture-{SESSION_ID}" in args
    assert "--collect" in args
    assert flag_values(args, "-p") == ["CPUWeight=100", "MemoryHigh=4G", f"BindsTo={GAME_UNIT}", f"After={GAME_UNIT}", "TimeoutStopSec=10"]
    assert "gpu-screen-recorder" in args
    assert flag_values(args, "-w") == ["DP-1"]
    assert flag_values(args, "-cursor") == ["yes"]
    assert flag_values(args, "-f") == ["30"]
    assert flag_values(args, "-k") == ["hevc"]
    assert flag_values(args, "-a") == ["default_output"]
    assert flag_values(args, "-ffmpeg-video-opts") == [QVBR_OPTS.replace("global_quality=95", "global_quality=22")]
    assert flag_values(args, "-o") == [pending_path(tmp_path)]


def test_start_fps_auto_takes_the_screens_refresh_rate(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"fps": "auto"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["120"]
    assert (fakebin["logs"] / "universe.args").read_text() == "screen-mode\nDP-1\n--json\n"

    env = env_for(tmp_path, fakebin, {"fps": "auto"}, extra={"SESSION_SCREEN": "HDMI-A-1"})
    assert run("start", env).returncode == 0
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["60"]


def test_start_fps_auto_falls_back_to_60_when_the_mode_is_unreadable(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"fps": "auto"}, extra={"FAKE_REFRESH": "0"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "fps auto" in result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["60"]

    env = env_for(tmp_path, fakebin, {"fps": "auto"}, extra={"FAKE_UNIVERSE_EXIT": "1"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "no shell" in result.stderr
    assert flag_values((fakebin["logs"] / "systemd-run.args").read_text().splitlines(), "-f") == ["60"]


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


def test_stop_short_recording_is_trashed(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "5"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "trash.args").read_text().splitlines() == [str(mkv)]
    assert not mkv.exists()
    assert not (fakebin["logs"] / "universe.args").exists()


def test_stop_long_recording_files_via_cli(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "universe.args").read_text().splitlines()
    assert args == ["recording-file", SESSION_ID, str(mkv)]
    assert mkv.exists()  # the fake CLI does not itself move the file


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


def test_shot_under_the_screenshots_dir(tmp_path, fakebin):
    result = run("shot", env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "screenshots"
    assert path.suffix == ".png"
    assert path.exists() and path.stat().st_size > 0


def test_shot_grabs_in_the_shell_when_the_extension_is_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "cursor": True}, extra={"FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "screenshots" and path.suffix == ".png"
    calls = (fakebin["logs"] / "busctl.args").read_text()
    assert f"Screenshot\nsbb\n{path}\ntrue\ntrue\n" in calls
    assert not (fakebin["logs"] / "gsr.args").exists()


def test_shot_screen_source_grabs_every_monitor(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen"}, extra={"FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert "\nsbb\n" + result.stdout.strip() + "\nfalse\nfalse\n" in (fakebin["logs"] / "busctl.args").read_text()


def test_shot_falls_back_to_gsr_when_the_shell_refuses(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"FAKE_NAME_OWNED": "true", "FAKE_SHOT_OK": "false"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert "shell screenshot" in result.stderr
    assert flag_values((fakebin["logs"] / "gsr.args").read_text().splitlines(), "-o") == [result.stdout.strip()]


WINDOW_JSON = '{"id":7,"pid":4242,"wm_class":"gamescope","title":"Dead Cells","focused":true,"width":3840,"height":2160,"hidden":false,"minimized":false}'


def test_start_window_records_through_the_portal_once_the_window_is_up(tmp_path, fakebin):
    env = env_for(
        tmp_path,
        fakebin,
        {"source": "window", "window_wait_s": 45},
        extra={"FAKE_NAME_OWNED": "true", "FAKE_WINDOW_JSON": WINDOW_JSON, "GAME_ID": "dead-cells"},
    )
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr

    assert (fakebin["logs"] / "universe.args").read_text() == "session-window\n--wait\n45\n--json\n"
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert f"--unit=universe-capture-{SESSION_ID}" in args
    assert f"BindsTo={GAME_UNIT}" in flag_values(args, "-p")
    assert flag_values(args, "-w") == ["portal"]
    assert flag_values(args, "-restore-portal-session") == ["yes"]
    assert flag_values(args, "-portal-session-token-filepath") == [str(tmp_path / "data" / "portal" / "dead-cells")]
    assert (tmp_path / "data" / "portal").is_dir()
    assert "ShowOSD" not in (fakebin["logs"] / "busctl.args").read_text()


def test_start_window_records_the_screen_when_no_window_shows_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 0}, extra={"FAKE_NAME_OWNED": "true", "GAME_ID": "dead-cells"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "no game window within 0s" in result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]
    assert "ShowOSD" in (fakebin["logs"] / "busctl.args").read_text()


def test_start_window_records_the_screen_when_the_cli_has_no_shell(tmp_path, fakebin):
    env = env_for(
        tmp_path, fakebin, {"source": "window", "window_wait_s": 0}, extra={"FAKE_NAME_OWNED": "true", "FAKE_UNIVERSE_EXIT": "1", "GAME_ID": "dead-cells"}
    )
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "no shell" in result.stderr and "no game window" in result.stderr
    assert flag_values((fakebin["logs"] / "systemd-run.args").read_text().splitlines(), "-w") == ["DP-1"]


def test_start_window_falls_back_to_gsr_when_extension_not_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window"}, extra={"FAKE_NAME_OWNED": "false"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "log out once" in result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]
    assert "ShowOSD" in (fakebin["logs"] / "busctl.args").read_text()


def test_extension_ready_guards(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CURRENT_DESKTOP", "GNOME")
    monkeypatch.setenv("HOME", str(tmp_path))
    monkeypatch.setenv("XDG_DATA_DIRS", str(tmp_path))
    assert not _common.extension_ready()
    install_fake_extension({"HOME": str(tmp_path)})
    monkeypatch.setenv("XDG_CURRENT_DESKTOP", "KDE")
    assert not _common.extension_ready()


def test_show_osd_passes_a_negative_level_past_busctl(tmp_path, fakebin):
    env = dict(os.environ, PATH=f"{fakebin['bin']}:{os.environ['PATH']}")
    result = subprocess.run(
        [sys.executable, "-c", "import _common; _common.show_osd('Recording the screen')"], cwd=BIN_DIR, env=env, capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "busctl.args").read_text().splitlines()
    assert args[args.index("ssd") + 1] == "--"
    assert args[-1] == "-1"


def test_start_screen_source_skips_the_extension(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen"}, extra={"FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert "gpu-screen-recorder" in args
    assert not (fakebin["logs"] / "busctl.args").exists()


def test_start_and_stop_follow_the_container(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"container": "mp4", "min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    final = tmp_path / "data" / "pending" / f"{SESSION_ID}.mp4"
    assert flag_values(args, "-o") == [str(final)]

    final.write_bytes(b"x")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "universe.args").read_text().splitlines()[:3] == ["recording-file", SESSION_ID, str(final)]


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
    gsr = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(gsr, "-ipc") == [_common.ipc_socket(SESSION_ID)] and flag_values(gsr, "-write-first-frame-ts") == ["yes"]


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
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    mkv = tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv"
    mkv.write_bytes(b"x")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert "gsr-cli stop" in result.stderr
    assert (fakebin["logs"] / "systemctl.args").read_text().splitlines()[-3:] == ["--user", "stop", f"universe-capture-{SESSION_ID}.service"]
    assert (fakebin["logs"] / "universe.args").read_text().splitlines()[:3] == ["recording-file", SESSION_ID, str(mkv)]


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


def test_stop_drops_the_timeline_with_a_short_recording(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "5"})
    assert run("start", env).returncode == 0
    (tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv").write_bytes(b"x")
    assert run("stop", env).returncode == 0
    assert _timeline(tmp_path) is None
    assert not (fakebin["logs"] / "universe.args").exists()


def test_codec_auto_takes_the_best_the_card_encodes(fakebin, monkeypatch):
    monkeypatch.setenv("PATH", f"{fakebin['bin']}:{os.environ.get('PATH', '')}")
    auto = _common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv")
    assert flag_values(auto, "-k") == ["av1_10bit"] and auto.count(QVBR_OPTS) == 1, "the AV1 quality preset goes with the AV1 codec"
    monkeypatch.setenv("FAKE_CODECS", "h264\nhevc")
    older = _common.gsr_args({"codec": "auto", "audio": "none"}, "DP-1", "/o.mkv")
    assert flag_values(older, "-k") == ["hevc"] and older[older.index("-ffmpeg-video-opts") + 1].startswith("rc_mode=QVBR;global_quality=22;")
    monkeypatch.setenv("FAKE_CODECS", "vp8")
    assert flag_values(_common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv"), "-k") == ["h264"], "nothing known: h264, the one every card has"
    assert flag_values(_common.gsr_args({"codec": "av1", "audio": "none"}, "DP-1", "/o.mkv"), "-k") == ["av1"], "a chosen codec is passed as is"


def test_gsr_args_quality_presets_and_overrides(fakebin, monkeypatch):
    monkeypatch.setenv("PATH", f"{fakebin['bin']}:{os.environ.get('PATH', '')}")
    hevc = _common.gsr_args({"codec": "hevc", "quality": "high", "audio": "none"}, "DP-1", "/o.mkv")
    assert hevc[hevc.index("-ffmpeg-video-opts") + 1] == "rc_mode=QVBR;global_quality=27;b=10000000;maxrate=20000000;bufsize=40000000"
    raw = _common.gsr_args({"quality": "ultra", "ffmpeg_video_opts": "rc_mode=CQP;qp=20", "audio": "none"}, "DP-1", "/o.mkv")
    assert raw[raw.index("-ffmpeg-video-opts") + 1] == "rc_mode=CQP;qp=20"
    assert _common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv").count(QVBR_OPTS) == 1


def test_gsr_args_container_size_audio_and_extra_args():
    args = _common.gsr_args(
        {"container": "mp4", "size": "2560x1440", "audio": "output", "audio_codec": "aac", "audio_bitrate": 160, "gsr_extra_args": "-cr full -keyint 2"},
        "DP-1",
        "/o.mp4",
    )
    assert flag_values(args, "-c") == ["mp4"]
    assert flag_values(args, "-s") == ["2560x1440"]
    assert flag_values(args, "-ac") == ["aac"]
    assert flag_values(args, "-ab") == ["160"]
    assert args[args.index("-cr") :] == ["-cr", "full", "-keyint", "2", "-o", "/o.mp4"]
    plain = _common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv")
    assert "-s" not in plain and "-ab" not in plain and "-a" not in plain and flag_values(plain, "-c") == ["mkv"]
    assert flag_values(plain, "-cursor") == ["no"]
    assert flag_values(_common.gsr_args({"audio": "output", "audio_codec": "flac"}, "DP-1", "/o.mkv"), "-ac") == ["opus"]


def test_size_limit_and_audio_bitrate_parse():
    assert _common.size_limit({"size": "1920x1080"}) == (1920, 1080)
    for raw in ("native", "", "0x0", "wide", None):
        assert _common.size_limit({"size": raw}) is None
    assert _common.audio_bitrate_kbps({"audio_bitrate": "128"}) == 128
    for raw in ("auto", "", 0, "0"):
        assert _common.audio_bitrate_kbps({"audio_bitrate": raw}) is None
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'a b'"}) == ["-x", "a b"]
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'unterminated"}) == []


def test_start_runs_the_recorder_under_record_with_the_hooks_env(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"GAME_ID": "sample"})
    assert run("start", env).returncode == 0
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    record = args.index(str(BIN_DIR / "record"))
    assert args[record + 1] == "--" and args[record + 2] == "gpu-screen-recorder"
    setenv = {a.split("=", 2)[1]: a.split("=", 2)[2] for a in args[:record] if a.startswith("--setenv=")}
    assert setenv["SESSION_ID"] == SESSION_ID and setenv["MODULE_DATA_DIR"] == env["MODULE_DATA_DIR"] and setenv["GAME_ID"] == "sample"
    assert setenv["PATH"] == env["PATH"], "gsr-cli and ffprobe are on the hook's PATH, not the manager's"
    assert "TERM" not in setenv


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


def _wait_for(pred, timeout_s=10):
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if pred():
            return True
        time.sleep(0.05)
    return False


def _apply_env(monkeypatch, env):
    """The supervisor runs in-process here: its children read the real environment."""
    for k, v in env.items():
        monkeypatch.setenv(k, v)


def _recorder(tmp_path, livebin, env, extra_args=()):
    output = pending_path(tmp_path)
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    argv = ["gpu-screen-recorder", "-w", "DP-1", "-f", "60", *extra_args, "-o", output]
    rec = _record.Recorder(argv, SESSION_ID, env["MODULE_DATA_DIR"], GAME_UNIT, drm_dir=str(livebin["drm"]))
    rec.poll_s = 0.05
    done = []
    thread = threading.Thread(target=lambda: done.append(rec.run()), daemon=True)
    return thread, done


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
    env = env_for(tmp_path, livebin, {})
    _apply_env(monkeypatch, env)
    thread, done = _recorder(tmp_path, livebin, env)
    thread.start()
    assert _wait_for((livebin["logs"] / "gsr.current").exists)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True):
        pass

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
    assert (livebin["logs"] / "busctl.args").read_text().count("Recording HDMI-A-1") == 1

    assert _end_session(env, thread) == pending_path(tmp_path)
    assert done == [0] and len(_gsr_runs(livebin)) == 2, "the session's own stop is not a switch"


def test_record_retries_while_the_new_monitor_settles(tmp_path, livebin, monkeypatch):
    env = env_for(tmp_path, livebin, {})
    _apply_env(monkeypatch, env)
    monkeypatch.setattr(_record, "RESTART_WAIT_S", 0.05)
    thread, done = _recorder(tmp_path, livebin, env)
    thread.start()
    assert _wait_for((livebin["logs"] / "gsr.current").exists)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True):
        pass

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
    env = env_for(tmp_path, livebin, {})
    _apply_env(monkeypatch, env)
    monkeypatch.setattr(_record, "RESTART_WAIT_S", 0.01)
    monkeypatch.setattr(_record, "RESTART_TRIES", 2)
    thread, done = _recorder(tmp_path, livebin, env)
    thread.start()
    assert _wait_for((livebin["logs"] / "gsr.current").exists)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True):
        pass
    (livebin["logs"] / "gsr.fail").touch()
    _plug(livebin, DP_1="disconnected", HDMI_A_1="connected")
    thread.join(timeout=10)
    assert done == [3], "the recorder's own exit code, as when it dies on a live monitor"
    assert len(_gsr_runs(livebin)) == 4
    state = _timeline(tmp_path)
    assert state["parts"] == [str(tmp_path / "data" / "pending" / f"{SESSION_ID}.part1.mkv")] and state["pauses"][0][1] is None


def test_record_exits_with_a_recorder_that_dies_on_a_live_monitor(tmp_path, livebin, monkeypatch):
    env = env_for(tmp_path, livebin, {})
    _apply_env(monkeypatch, env)
    thread, done = _recorder(tmp_path, livebin, env)
    thread.start()
    assert _wait_for((livebin["logs"] / "gsr.current").exists)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True):
        pass
    Path(pending_path(tmp_path) + ".stopflag").touch()
    thread.join(timeout=10)
    assert done == [0] and len(_gsr_runs(livebin)) == 1
    assert "parts" not in _timeline(tmp_path)


def test_record_pauses_a_restarted_recorder_while_the_game_is_frozen(tmp_path, livebin, monkeypatch):
    env = env_for(tmp_path, livebin, {}, extra={"FAKE_FREEZER_STATE": "frozen"})
    _apply_env(monkeypatch, env)
    thread, done = _recorder(tmp_path, livebin, env)
    thread.start()
    assert _wait_for((livebin["logs"] / "gsr.current").exists)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True) as state:
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


def test_shot_falls_back_to_the_screen_the_recorder_moved_to(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"FAKE_SHOT_OK": "false", "FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    with _common.timeline(env["MODULE_DATA_DIR"], SESSION_ID, create=True) as state:
        state["screen"] = "HDMI-A-1"
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert flag_values((fakebin["logs"] / "gsr.args").read_text().splitlines(), "-w") == ["HDMI-A-1"]


def test_connected_outputs_and_argv_edits(tmp_path):
    drm = tmp_path / "drm"
    for name, status in (("card1-DP-1", "connected"), ("card1-HDMI-A-1", "disconnected"), ("card0-DP-3", "connected")):
        (drm / name).mkdir(parents=True)
        (drm / name / "status").write_text(status + "\n")
    (drm / "card1").mkdir()
    assert _common.connected_outputs(str(drm)) == ["DP-1", "DP-3"]
    assert _common.connected_outputs(str(tmp_path / "nope")) == []
    argv = ["gpu-screen-recorder", "-w", "DP-1", "-o", "out.mkv"]
    assert _common.with_flag(argv, "-w", "HDMI-A-1") == ["gpu-screen-recorder", "-w", "HDMI-A-1", "-o", "out.mkv"]
    assert _common.with_flag(argv, "-s", "1920x1080") == ["gpu-screen-recorder", "-w", "DP-1", "-s", "1920x1080", "-o", "out.mkv"]
    assert _common.flag_value(argv, "-o") == "out.mkv" and _common.flag_value(argv, "-s") is None
    assert _common.part_path("/p/20260911-120000.mkv", 2) == "/p/20260911-120000.part2.mkv"
