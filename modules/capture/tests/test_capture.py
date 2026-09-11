"""Runs bin/start, bin/stop, bin/shot as subprocesses against fake systemd-run /
systemctl / busctl / ffprobe / gpu-screen-recorder / trash shims on PATH."""
import json
import os
import stat
import subprocess
from pathlib import Path

import pytest

MODULE_DIR = Path(__file__).resolve().parents[1]
BIN_DIR = MODULE_DIR / "bin"

QVBR_OPTS = "rc_mode=QVBR;global_quality=95;b=16000000;maxrate=32000000;bufsize=64000000"
BUS = "io.github.ilyasturki.Universe"
OBJECT = "/io/github/ilyasturki/Universe"
SESSION_ID = "20260911-120000"


def _write_shim(path, body):
    path.write_text(f"#!/usr/bin/env bash\n{body}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


@pytest.fixture
def fakebin(tmp_path):
    bindir = tmp_path / "fakebin"
    bindir.mkdir()
    logs = tmp_path / "calls"
    logs.mkdir()

    _write_shim(bindir / "systemd-run", f'printf "%s\\n" "$@" > "{logs}/systemd-run.args"\nexit 0\n')
    _write_shim(bindir / "systemctl", f'''printf "%s\\n" "$@" >> "{logs}/systemctl.args"
if [ "$1" = "stop" ]; then exit "${{FAKE_SYSTEMCTL_STOP_EXIT:-0}}"; fi
if [ "$1" = "is-active" ]; then echo "${{FAKE_IS_ACTIVE:-active}}"; exit 0; fi
exit 0''')
    _write_shim(bindir / "busctl", f'''printf "%s\\n" "$@" > "{logs}/busctl.args"
if [ "${{FAKE_BUSCTL_EXIT:-0}}" != "0" ]; then echo "Failed to activate service" >&2; exit "${{FAKE_BUSCTL_EXIT}}"; fi
echo "s \\"/mnt/recordings/games/fake/session.mkv\\""
exit 0''')
    _write_shim(bindir / "ffprobe", 'echo "${FAKE_DURATION:-300}"\nexit 0\n')
    _write_shim(bindir / "trash", f'printf "%s\\n" "$@" > "{logs}/trash.args"\nrm -f "$1"\nexit 0\n')
    _write_shim(bindir / "gpu-screen-recorder", f'''printf "%s\\n" "$@" >> "{logs}/gsr.args"
if [ "$1" = "--list-monitors" ]; then echo "${{FAKE_MONITOR:-DP-1|3840x2160}}"; exit 0; fi
for ((i=1; i<=$#; i++)); do
  if [ "${{!i}}" = "-o" ]; then j=$((i+1)); echo fake > "${{!j}}"; fi
done
exit 0''')
    return {"bin": bindir, "logs": logs}


def env_for(tmp_path, fakebin, settings, session_id=SESSION_ID, extra=None):
    env = dict(os.environ)
    env["PATH"] = f"{fakebin['bin']}:{env.get('PATH', '')}"
    env["SESSION_ID"] = session_id
    env["MODULE_DATA_DIR"] = str(tmp_path / "data")
    env["MODULE_DIR"] = str(MODULE_DIR)
    env["UNIVERSE_BUS"] = BUS
    env["UNIVERSE_OBJECT"] = OBJECT
    env["MODULE_SETTINGS_JSON"] = json.dumps(settings)
    env.setdefault("SESSION_SCREEN", "DP-1")
    if extra:
        env.update(extra)
    return env


def run(script, env):
    return subprocess.run([str(BIN_DIR / script)], env=env, capture_output=True, text=True, timeout=30)


def flag_values(args, flag):
    return [args[i + 1] for i, a in enumerate(args) if a == flag]


def pending_path(tmp_path, session_id=SESSION_ID):
    return str(tmp_path / "data" / "pending" / f"{session_id}.mkv")


# --- bin/start ---------------------------------------------------------

def test_start_composes_gsr_command(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {
        "enabled": True, "cursor": True, "codec": "hevc", "fps": 30, "audio": "output",
    })
    result = run("start", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert f"--unit=universe-capture-{SESSION_ID}" in args
    assert "--collect" in args
    assert flag_values(args, "-p") == ["CPUWeight=100", "MemoryHigh=4G", "TimeoutStopSec=10"]
    assert "gpu-screen-recorder" in args
    assert flag_values(args, "-w") == ["DP-1"]
    assert flag_values(args, "-cursor") == ["yes"]
    assert flag_values(args, "-f") == ["30"]
    assert flag_values(args, "-k") == ["hevc"]
    assert flag_values(args, "-a") == ["default_output"]
    assert flag_values(args, "-ffmpeg-video-opts") == [QVBR_OPTS]
    assert flag_values(args, "-o") == [pending_path(tmp_path)]


def test_start_cursor_off_and_audio_both(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"cursor": False, "audio": "output+input"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-cursor") == ["no"]
    assert flag_values(args, "-a") == ["default_output", "default_input"]


def test_start_audio_none_omits_a_flag(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"audio": "none"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert "-a" not in args


def test_start_custom_quality_passthrough(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"quality": "rc_mode=CQP;qp=20"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-ffmpeg-video-opts") == ["rc_mode=CQP;qp=20"]


def test_start_resolves_screen_when_unset(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"SESSION_SCREEN": ""})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]


def test_start_enabled_false_exits_early(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"enabled": False})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "systemd-run.args").exists()
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


# --- bin/stop ------------------------------------------------------------

def _seed_pending(tmp_path, session_id=SESSION_ID, with_companion=True):
    pending = tmp_path / "data" / "pending"
    pending.mkdir(parents=True)
    mkv = pending / f"{session_id}.mkv"
    mkv.write_bytes(b"fake mkv contents")
    if with_companion:
        (pending / f"{session_id}.json").write_text("{}")
    return mkv


def test_stop_short_recording_is_trashed(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "5"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "trash.args").read_text().splitlines() == [str(mkv)]
    assert not mkv.exists()
    assert not (fakebin["logs"] / "busctl.args").exists()
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_long_recording_files_via_dbus(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "busctl.args").read_text().splitlines()
    assert args == [
        "--user", "call", BUS, OBJECT,
        "io.github.ilyasturki.Universe.Recording1", "File", "ss",
        SESSION_ID, str(mkv),
    ]
    assert mkv.exists()  # the fake bus call does not itself move the file
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_dbus_failure_leaves_file_and_exits_nonzero(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999", "FAKE_BUSCTL_EXIT": "1"})
    result = run("stop", env)
    assert result.returncode == 1
    assert "left in pending" in result.stderr
    assert mkv.exists()
    assert (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_no_recording_is_a_noop(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "busctl.args").exists()


# --- bin/shot --------------------------------------------------------------

def test_shot_under_journal_dir_attachments(tmp_path, fakebin):
    journal_dir = tmp_path / "games" / "some-game" / "journal"
    env = env_for(tmp_path, fakebin, {}, extra={"JOURNAL_DIR": str(journal_dir)})
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == journal_dir / "attachments"
    assert path.suffix == ".png"
    assert path.exists() and path.stat().st_size > 0


def test_shot_falls_back_to_module_data_dir(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"JOURNAL_DIR": ""})
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "data" / "screenshots"
    assert path.exists()
