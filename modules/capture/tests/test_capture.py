import importlib.util
import json
import os
import shutil
import stat
import subprocess
import sys
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
    _write_shim(bindir / "systemctl", f'''printf "%s\\n" "$@" >> "{logs}/systemctl.args"
case "$*" in
  *FreezerState*) echo "${{FAKE_FREEZER_STATE:-running}}";;
esac
exit "${{FAKE_KILL_EXIT:-0}}"''')
    _write_shim(bindir / "universe", f'''printf "%s\\n" "$@" >> "{logs}/universe.args"
if [ "${{FAKE_UNIVERSE_EXIT:-0}}" != "0" ]; then echo "universe: unavailable: no shell" >&2; exit "${{FAKE_UNIVERSE_EXIT}}"; fi
case "$1" in
  screen-mode) hz="${{FAKE_REFRESH:-}}"; [ -n "$hz" ] || {{ [ "$2" = HDMI-A-1 ] && hz=60 || hz=120; }}
    echo "{{\\"screen\\":\\"$2\\",\\"width\\":3840,\\"height\\":2160,\\"refresh\\":$hz}}"; exit 0;;
  session-window) echo "${{FAKE_WINDOW_JSON:-null}}"; exit 0;;
esac
echo "/mnt/recordings/games/fake/session.mkv"
exit 0''')
    _write_shim(bindir / "ffprobe", 'echo "${FAKE_DURATION:-300}"\nexit 0\n')
    _write_shim(bindir / "busctl", f'''printf "%s\\n" "$@" >> "{logs}/busctl.args"
case "$*" in
  *NameHasOwner*) echo "b ${{FAKE_NAME_OWNED:-false}}"; exit 0;;
  *EnableExtension*) echo "b ${{FAKE_NAME_OWNED:-false}}"; exit 0;;
  *" Screenshot "*) [ "${{FAKE_SHOT_OK:-true}}" = true ] && echo fake > "$8"; echo "b ${{FAKE_SHOT_OK:-true}}"; exit 0;;
esac
exit 0''')
    _write_shim(bindir / "trash", f'printf "%s\\n" "$@" > "{logs}/trash.args"\nrm -f "$1"\nexit 0\n')
    _write_shim(bindir / "gpu-screen-recorder", f'''printf "%s\\n" "$@" >> "{logs}/gsr.args"
for ((i=1; i<=$#; i++)); do
  if [ "${{!i}}" = "-o" ]; then j=$((i+1)); echo fake > "${{!j}}"; fi
done
exit 0''')
    return {"bin": bindir, "logs": logs}


def env_for(tmp_path, fakebin, settings, extra=None):
    env = dict(os.environ)
    env["PATH"] = f"{fakebin['bin']}:{env.get('PATH', '')}"
    env["SESSION_ID"] = SESSION_ID
    env["SESSION_UNIT"] = GAME_UNIT
    env["MODULE_DATA_DIR"] = str(tmp_path / "data")
    env["JOURNAL_DIR"] = str(tmp_path / "journal")
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
    return subprocess.run([str(BIN_DIR / script)], env=env, capture_output=True, text=True, timeout=30)


def flag_values(args, flag):
    return [args[i + 1] for i, a in enumerate(args) if a == flag]


def pending_path(tmp_path):
    return str(tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv")


def test_start_composes_gsr_command(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {
        "enabled": True, "cursor": True, "codec": "hevc", "fps": 30, "audio": "output",
    })
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
        result = subprocess.run([str(BIN_DIR / "choices"), "fps"], env=env, capture_output=True, text=True, timeout=30)
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


def _seed_pending(tmp_path):
    pending = tmp_path / "data" / "pending"
    pending.mkdir(parents=True)
    mkv = pending / f"{SESSION_ID}.mkv"
    mkv.write_bytes(b"fake mkv contents")
    return mkv


def test_stop_short_recording_is_trashed(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "5"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "trash.args").read_text().splitlines() == [str(mkv)]
    assert not mkv.exists()
    assert not (fakebin["logs"] / "universe.args").exists()


def test_stop_long_recording_files_via_cli(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "universe.args").read_text().splitlines()
    assert args == ["recording-file", SESSION_ID, str(mkv)]
    assert mkv.exists()  # the fake CLI does not itself move the file


def test_stop_cli_failure_leaves_file_and_exits_nonzero(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999", "FAKE_UNIVERSE_EXIT": "1"})
    result = run("stop", env)
    assert result.returncode == 1
    assert "left in pending" in result.stderr
    assert mkv.exists()


def test_stop_no_recording_is_a_noop(tmp_path, fakebin):
    result = run("stop", env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "universe.args").exists()


def test_shot_under_journal_dir_attachments(tmp_path, fakebin):
    result = run("shot", env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "journal" / "attachments"
    assert path.suffix == ".png"
    assert path.exists() and path.stat().st_size > 0


def test_shot_grabs_in_the_shell_when_the_extension_is_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "cursor": True}, extra={"FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "journal" / "attachments" and path.suffix == ".png"
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
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 45},
                  extra={"FAKE_NAME_OWNED": "true", "FAKE_WINDOW_JSON": WINDOW_JSON, "GAME_ID": "dead-cells"})
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
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 0},
                  extra={"FAKE_NAME_OWNED": "true", "GAME_ID": "dead-cells"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "no game window within 0s" in result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]
    assert "ShowOSD" in (fakebin["logs"] / "busctl.args").read_text()


def test_start_window_records_the_screen_when_the_cli_has_no_shell(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 0},
                  extra={"FAKE_NAME_OWNED": "true", "FAKE_UNIVERSE_EXIT": "1", "GAME_ID": "dead-cells"})
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
        [sys.executable, "-c", "import _common; _common.show_osd('Recording the screen')"],
        cwd=BIN_DIR, env=env, capture_output=True, text=True)
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


def _signals(fakebin):
    lines = (fakebin["logs"] / "systemctl.args").read_text().splitlines() if (fakebin["logs"] / "systemctl.args").exists() else []
    return lines.count("--signal=SIGUSR2")


def test_freeze_before_start_is_a_noop_and_start_catches_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("freeze", env).returncode == 0
    assert _timeline(tmp_path) is None and _signals(fakebin) == 0

    result = run("start", dict(env, FAKE_FREEZER_STATE="frozen"))
    assert result.returncode == 0, result.stderr
    state = _timeline(tmp_path)
    assert state["paused"] and len(state["pauses"]) == 1 and state["pauses"][0][1] is None
    assert state["started_at"] <= state["pauses"][0][0]
    assert _signals(fakebin) == 1
    args = (fakebin["logs"] / "systemctl.args").read_text().splitlines()
    assert args[args.index("--signal=SIGUSR2") - 1] == "--kill-whom=main"
    assert args[args.index("--signal=SIGUSR2") + 1] == f"universe-capture-{SESSION_ID}.service"


def test_freeze_and_thaw_toggle_the_recorder_once_each(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("start", env).returncode == 0
    assert _timeline(tmp_path) == {"started_at": _timeline(tmp_path)["started_at"], "paused": False, "pauses": []}
    assert _signals(fakebin) == 0

    assert run("freeze", env).returncode == 0
    assert run("freeze", env).returncode == 0
    assert _signals(fakebin) == 1 and _timeline(tmp_path)["paused"]

    assert run("thaw", env).returncode == 0
    assert run("thaw", env).returncode == 0
    state = _timeline(tmp_path)
    assert _signals(fakebin) == 2 and not state["paused"]
    assert len(state["pauses"]) == 1 and state["pauses"][0][0] <= state["pauses"][0][1]


def test_freeze_keeps_its_state_when_the_signal_fails(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    assert run("start", env).returncode == 0
    result = run("freeze", dict(env, FAKE_KILL_EXIT="1"))
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
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    handed = json.loads((tmp_path / "handed.json").read_text())
    assert not handed["paused"] and len(handed["pauses"]) == 1 and handed["pauses"][0][1] is not None
    assert not timeline.exists() and not (tmp_path / "data" / "pending" / f"{SESSION_ID}.timeline.json.lock").exists()


def test_stop_drops_the_timeline_with_a_short_recording(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240}, extra={"FAKE_DURATION": "5"})
    assert run("start", env).returncode == 0
    (tmp_path / "data" / "pending" / f"{SESSION_ID}.mkv").write_bytes(b"x")
    assert run("stop", env).returncode == 0
    assert _timeline(tmp_path) is None
    assert not (fakebin["logs"] / "universe.args").exists()


def test_gsr_args_quality_presets_and_overrides():
    hevc = _common.gsr_args({"codec": "hevc", "quality": "high", "audio": "none"}, "DP-1", "/o.mkv")
    assert hevc[hevc.index("-ffmpeg-video-opts") + 1] == "rc_mode=QVBR;global_quality=27;b=10000000;maxrate=20000000;bufsize=40000000"
    raw = _common.gsr_args({"quality": "ultra", "ffmpeg_video_opts": "rc_mode=CQP;qp=20", "audio": "none"}, "DP-1", "/o.mkv")
    assert raw[raw.index("-ffmpeg-video-opts") + 1] == "rc_mode=CQP;qp=20"
    assert _common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv").count(QVBR_OPTS) == 1


def test_gsr_args_container_size_audio_and_extra_args():
    args = _common.gsr_args({"container": "mp4", "size": "2560x1440", "audio": "output", "audio_codec": "aac",
                             "audio_bitrate": 160, "gsr_extra_args": "-cr full -keyint 2"}, "DP-1", "/o.mp4")
    assert flag_values(args, "-c") == ["mp4"]
    assert flag_values(args, "-s") == ["2560x1440"]
    assert flag_values(args, "-ac") == ["aac"]
    assert flag_values(args, "-ab") == ["160"]
    assert args[args.index("-cr"):] == ["-cr", "full", "-keyint", "2", "-o", "/o.mp4"]
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
