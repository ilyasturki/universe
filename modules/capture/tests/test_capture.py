"""Runs bin/start, bin/stop, bin/shot as subprocesses against fake systemd-run /
systemctl / universe / ffprobe / gpu-screen-recorder / trash shims on PATH."""
import importlib.machinery
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
if [ "$1" = "stop" ]; then exit "${{FAKE_SYSTEMCTL_STOP_EXIT:-0}}"; fi
if [ "$1" = "is-active" ]; then echo "${{FAKE_IS_ACTIVE:-active}}"; exit 0; fi
if [ "$2" = "show" ]; then echo "${{FAKE_CGROUP:-}}"; exit 0; fi
exit 0''')
    _write_shim(bindir / "universe", f'''printf "%s\\n" "$@" > "{logs}/universe.args"
if [ "${{FAKE_UNIVERSE_EXIT:-0}}" != "0" ]; then echo "universe: not found: session" >&2; exit "${{FAKE_UNIVERSE_EXIT}}"; fi
echo "/mnt/recordings/games/fake/session.mkv"
exit 0''')
    _write_shim(bindir / "ffprobe", 'echo "${FAKE_DURATION:-300}"\nexit 0\n')
    _write_shim(bindir / "ffmpeg", f'''printf "%s\\n" "$@" >> "{logs}/ffmpeg.args"
out="${{@: -1}}"
echo fake > "$out"
exit 0''')
    # busctl --json=short as Mutter answers GetCurrentState (DP-1 at 120 Hz, HDMI-A-1 at 60 Hz) and the
    # extension List (one 4K window owned by the calling hook, hidden on demand).
    _write_shim(bindir / "busctl", f'''printf "%s\\n" "$@" >> "{logs}/busctl.args"
case "$*" in
  *NameHasOwner*) echo "b ${{FAKE_NAME_OWNED:-false}}"; exit 0;;
  *EnableExtension*) echo "b true"; exit 0;;
  *" List") printf '{{"type":"s","data":["[{{\\\\"id\\\\":7,\\\\"pid\\\\":%s,\\\\"width\\\\":3840,\\\\"height\\\\":2160,\\\\"hidden\\\\":%s}}]"]}}\\n' "$PPID" "${{FAKE_WINDOW_HIDDEN:-false}}"; exit 0;;
  *" Screenshot "*) [ "${{FAKE_SHOT_OK:-true}}" = true ] && echo fake > "$8"; echo "b ${{FAKE_SHOT_OK:-true}}"; exit 0;;
esac
if [ "${{FAKE_BUSCTL_EXIT:-0}}" != "0" ]; then exit "${{FAKE_BUSCTL_EXIT}}"; fi
echo '{{"type":"ua((ssss)a(siiddada{{sv}})a{{sv}})a(iiduba(ssss)a{{sv}})a{{sv}}","data":[1,[[["DP-1","GSM","LG","0x1"],[["3840x2160@59.997",3840,2160,59.997,1.5,[1.0],{{}}],["3840x2160@119.88",3840,2160,119.88,1.5,[1.0],{{"is-current":{{"type":"b","data":true}}}}]],{{}}],[["HDMI-A-1","DEL","Dell","0x2"],[["2560x1440@59.951",2560,1440,59.951,1.0,[1.0],{{"is-current":{{"type":"b","data":true}}}}]],{{}}]],[],{{}}]}}'
exit 0''')
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
    env["UNIVERSE_BIN"] = str(fakebin["bin"] / "universe")
    env["MODULE_SETTINGS_JSON"] = json.dumps(settings)
    env.setdefault("SESSION_SCREEN", "DP-1")
    # Deterministic across hosts: no shell extension unless a test installs one.
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
    assert flag_values(args, "-ffmpeg-video-opts") == [QVBR_OPTS.replace("global_quality=95", "global_quality=22")]
    assert flag_values(args, "-o") == [pending_path(tmp_path)]


def test_start_binds_recorder_to_game_unit(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"enabled": True})
    env["SESSION_UNIT"] = "universe-game-x-1.service"
    result = run("start", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-p") == [
        "CPUWeight=100", "MemoryHigh=4G", "BindsTo=universe-game-x-1.service", "After=universe-game-x-1.service", "TimeoutStopSec=10",
    ]


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


def test_start_fps_auto_takes_the_screens_refresh_rate(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"fps": "auto"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["120"]
    assert "GetCurrentState" in (fakebin["logs"] / "busctl.args").read_text()

    env = env_for(tmp_path, fakebin, {"fps": "auto"}, extra={"SESSION_SCREEN": "HDMI-A-1"})
    assert run("start", env).returncode == 0
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["60"]


def test_start_fps_auto_falls_back_to_60_without_mutter(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"fps": "auto"}, extra={"FAKE_BUSCTL_EXIT": "1"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "fps auto" in result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-f") == ["60"]


def test_fps_choices_stop_at_the_screens_refresh_rate(tmp_path, fakebin):
    def choices(**extra):
        result = subprocess.run([str(BIN_DIR / "choices"), "fps"], env=env_for(tmp_path, fakebin, {}, extra=extra), capture_output=True, text=True, timeout=30)
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)

    assert choices(SESSION_SCREEN="HDMI-A-1") == ["auto", "60", "30"]
    assert choices(SESSION_SCREEN="DP-1") == ["auto", "120", "90", "60", "30"]
    assert choices(FAKE_BUSCTL_EXIT="1") == ["auto", "120", "90", "60", "30"], "no mutter: every rate stays"


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
    assert not (fakebin["logs"] / "universe.args").exists()
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_long_recording_files_via_cli(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999"})
    result = run("stop", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "universe.args").read_text().splitlines()
    assert args == ["recording-file", SESSION_ID, str(mkv)]
    assert mkv.exists()  # the fake CLI does not itself move the file
    assert not (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_cli_failure_leaves_file_and_exits_nonzero(tmp_path, fakebin):
    mkv = _seed_pending(tmp_path)
    env = env_for(tmp_path, fakebin, {"min_duration_s": 240},
                   extra={"FAKE_DURATION": "999", "FAKE_UNIVERSE_EXIT": "1"})
    result = run("stop", env)
    assert result.returncode == 1
    assert "left in pending" in result.stderr
    assert mkv.exists()
    assert (tmp_path / "data" / "pending" / f"{SESSION_ID}.json").exists()


def test_stop_no_recording_is_a_noop_and_drops_the_companion(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {})
    companion = Path(env["MODULE_DATA_DIR"]) / "pending" / f"{SESSION_ID}.json"
    companion.parent.mkdir(parents=True, exist_ok=True)
    companion.write_text("{}")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "universe.args").exists()
    assert not companion.exists()


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


def test_shot_grabs_in_the_shell_when_the_extension_is_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "cursor": True},
                  extra={"JOURNAL_DIR": "", "FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "data" / "screenshots" and path.suffix == ".png"
    calls = (fakebin["logs"] / "busctl.args").read_text()
    assert f"Screenshot\nsbb\n{path}\ntrue\ntrue\n" in calls
    assert not (fakebin["logs"] / "gsr.args").exists()


def test_shot_screen_source_grabs_every_monitor(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen"}, extra={"JOURNAL_DIR": "", "FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert "\nsbb\n" + result.stdout.strip() + "\nfalse\nfalse\n" in (fakebin["logs"] / "busctl.args").read_text()


def test_shot_falls_back_to_gsr_when_the_shell_refuses(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"JOURNAL_DIR": "", "FAKE_NAME_OWNED": "true", "FAKE_SHOT_OK": "false"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert "shell screenshot" in result.stderr
    assert flag_values((fakebin["logs"] / "gsr.args").read_text().splitlines(), "-o") == [result.stdout.strip()]


def test_shot_falls_back_to_gsr_when_the_extension_is_not_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {}, extra={"JOURNAL_DIR": "", "FAKE_NAME_OWNED": "false"})
    install_fake_extension(env)
    result = run("shot", env)
    assert result.returncode == 0, result.stderr
    assert "not loaded" in result.stderr
    assert (fakebin["logs"] / "gsr.args").exists()


# --- bin/start window mode ------------------------------------------------

def own_cgroup():
    return Path("/proc/self/cgroup").read_text().strip().split("::", 1)[1]


def test_start_window_records_through_the_portal_once_the_window_is_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window"},
                  extra={"FAKE_NAME_OWNED": "true", "SESSION_UNIT": "universe-game-x-1.service",
                         "FAKE_CGROUP": own_cgroup(), "GAME_ID": "dead-cells"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert f"--unit=universe-capture-{SESSION_ID}" in args
    assert "BindsTo=universe-game-x-1.service" in flag_values(args, "-p")
    assert flag_values(args, "-w") == ["portal"]
    assert flag_values(args, "-restore-portal-session") == ["yes"]
    assert flag_values(args, "-portal-session-token-filepath") == [str(tmp_path / "data" / "portal" / "dead-cells")]
    assert (tmp_path / "data" / "portal").is_dir()
    assert "ShowOSD" not in (fakebin["logs"] / "busctl.args").read_text()
    companion = json.loads((tmp_path / "data" / "pending" / f"{SESSION_ID}.json").read_text())
    assert companion["mode"] == "window"


def test_start_window_records_the_screen_when_no_window_shows_up(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window", "window_wait_s": 1},
                  extra={"FAKE_NAME_OWNED": "true", "SESSION_UNIT": "universe-game-x-1.service",
                         "FAKE_CGROUP": own_cgroup(), "FAKE_WINDOW_HIDDEN": "true", "GAME_ID": "dead-cells"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "no game window within 1s" in result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]
    assert "ShowOSD" in (fakebin["logs"] / "busctl.args").read_text()
    companion = json.loads((tmp_path / "data" / "pending" / f"{SESSION_ID}.json").read_text())
    assert companion["mode"] == "screen"


def test_pick_window_wants_a_visible_toplevel_of_the_unit():
    mine = {"id": 1, "pid": os.getpid(), "width": 100, "height": 100}
    bigger = dict(mine, id=2, width=200)
    assert _common.pick_window([mine, bigger], own_cgroup())["id"] == 2
    assert _common.pick_window([dict(mine, hidden=True)], own_cgroup()) is None
    assert _common.pick_window([dict(mine, minimized=True)], own_cgroup()) is None
    # Not pid 1: in a build sandbox every process shares the root cgroup.
    assert _common.pick_window([dict(mine, pid=2**62)], own_cgroup()) is None
    assert _common.pick_window([mine], None) is None


def test_start_window_falls_back_to_gsr_when_extension_not_loaded(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window"}, extra={"FAKE_NAME_OWNED": "false"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert "log out once" in result.stderr

    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert flag_values(args, "-w") == ["DP-1"]
    companion = json.loads((tmp_path / "data" / "pending" / f"{SESSION_ID}.json").read_text())
    assert companion["mode"] == "screen"
    # The fallback is said on screen, not only in the log.
    assert "ShowOSD" in (fakebin["logs"] / "busctl.args").read_text()


def test_start_screen_source_shows_no_osd(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    assert not (fakebin["logs"] / "busctl.args").exists()


def test_show_osd_passes_a_negative_level_past_busctl(tmp_path, fakebin):
    env = dict(os.environ, PATH=f"{fakebin['bin']}:{os.environ['PATH']}")
    result = subprocess.run(
        [sys.executable, "-c", "import _common; _common.show_osd('Recording the screen')"],
        cwd=BIN_DIR, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "busctl.args").read_text().splitlines()
    assert args[args.index("ssd") + 1] == "--"
    assert args[-1] == "-1"


def test_start_window_falls_back_to_gsr_when_extension_missing(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window"})
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert "gpu-screen-recorder" in args


def test_start_screen_source_skips_the_extension(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "screen"}, extra={"FAKE_NAME_OWNED": "true"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert "gpu-screen-recorder" in args
    busctl_log = fakebin["logs"] / "busctl.args"
    assert "NameHasOwner" not in (busctl_log.read_text() if busctl_log.exists() else "")


def test_start_window_not_on_gnome_uses_gsr(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"source": "window"}, extra={"XDG_CURRENT_DESKTOP": "KDE"})
    install_fake_extension(env)
    result = run("start", env)
    assert result.returncode == 0, result.stderr
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    assert "gpu-screen-recorder" in args


def test_start_and_stop_follow_the_container(tmp_path, fakebin):
    env = env_for(tmp_path, fakebin, {"container": "mp4", "min_duration_s": 240}, extra={"FAKE_DURATION": "999"})
    assert run("start", env).returncode == 0
    args = (fakebin["logs"] / "systemd-run.args").read_text().splitlines()
    final = tmp_path / "data" / "pending" / f"{SESSION_ID}.mp4"
    assert flag_values(args, "-o") == [str(final)]
    companion = json.loads((tmp_path / "data" / "pending" / f"{SESSION_ID}.json").read_text())
    assert companion["output"] == str(final)

    final.write_bytes(b"x")
    result = run("stop", env)
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "universe.args").read_text().splitlines() == ["recording-file", SESSION_ID, str(final)]


def test_gsr_args_flac_falls_back_to_opus():
    args = _common.gsr_args({"audio": "output", "audio_codec": "flac"}, "DP-1", "/o.mkv")
    assert flag_values(args, "-ac") == ["opus"]


# --- pure helpers ----------------------------------------------------------

def test_gsr_args_matches_the_screen_path():
    args = _common.gsr_args(
        {"cursor": True, "codec": "hevc", "fps": 30, "audio": "output", "quality": "qvbr"},
        "DP-1", "/out.mkv")
    assert args[0] == "gpu-screen-recorder"
    assert args[args.index("-w") + 1] == "DP-1"
    assert args[args.index("-cursor") + 1] == "yes"
    assert args[args.index("-f") + 1] == "30"
    assert args[args.index("-k") + 1] == "hevc"
    assert args[args.index("-o") + 1] == "/out.mkv"


def test_gsr_args_quality_presets_and_overrides():
    hevc = _common.gsr_args({"codec": "hevc", "quality": "high", "audio": "none"}, "DP-1", "/o.mkv")
    assert hevc[hevc.index("-ffmpeg-video-opts") + 1] == "rc_mode=QVBR;global_quality=27;b=10000000;maxrate=20000000;bufsize=40000000"
    raw = _common.gsr_args({"quality": "ultra", "ffmpeg_video_opts": "rc_mode=CQP;qp=20", "audio": "none"}, "DP-1", "/o.mkv")
    assert raw[raw.index("-ffmpeg-video-opts") + 1] == "rc_mode=CQP;qp=20"
    assert _common.gsr_args({"quality": "qvbr", "audio": "none"}, "DP-1", "/o.mkv").count(QVBR_OPTS) == 1


def test_gsr_args_container_size_audio_and_extra_args():
    args = _common.gsr_args({"container": "mp4", "size": "2560x1440", "audio": "output", "audio_codec": "aac",
                             "audio_bitrate": 160, "gsr_extra_args": "-cr full -keyint 2"}, "DP-1", "/o.mp4")
    assert flag_values(args, "-c") == ["mp4"]
    assert flag_values(args, "-s") == ["2560x1440"]
    assert flag_values(args, "-ac") == ["aac"]
    assert flag_values(args, "-ab") == ["160"]
    assert args[args.index("-cr"):] == ["-cr", "full", "-keyint", "2", "-o", "/o.mp4"]
    plain = _common.gsr_args({"audio": "none"}, "DP-1", "/o.mkv")
    assert "-s" not in plain and "-ab" not in plain and flag_values(plain, "-c") == ["mkv"]


def test_size_limit_and_audio_bitrate_parse():
    assert _common.size_limit({"size": "1920x1080"}) == (1920, 1080)
    for raw in ("native", "", "0x0", "wide", None):
        assert _common.size_limit({"size": raw}) is None
    assert _common.audio_bitrate_kbps({"audio_bitrate": "128"}) == 128
    for raw in ("auto", "", 0, "0", "loud"):
        assert _common.audio_bitrate_kbps({"audio_bitrate": raw}) is None
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'a b'"}) == ["-x", "a b"]
    assert _common.gsr_extra_args({"gsr_extra_args": "-x 'unterminated"}) == []


def test_output_path_follows_the_container():
    assert _common.output_path("/p", "s", {"container": "mp4"}) == "/p/s.mp4"
    assert _common.output_path("/p", "s", {}) == "/p/s.mkv"


def test_window_wait_s_setting():
    assert _common.window_wait_s({}) == 60
    assert _common.window_wait_s({"window_wait_s": "120"}) == 120
    assert _common.window_wait_s({"window_wait_s": -5}) == 0
    assert _common.window_wait_s({"window_wait_s": "soon"}) == 60


def test_cgroup_matches():
    unit_cg = "/user.slice/user-1000.slice/user@1000.service/app.slice/universe-game-x.service"
    exact = f"0::{unit_cg}\n"
    child = f"0::{unit_cg}/sub\n"
    other = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/other.service\n"
    assert _common.cgroup_matches(exact, unit_cg)
    assert _common.cgroup_matches(child, unit_cg)
    assert not _common.cgroup_matches(other, unit_cg)
    assert not _common.cgroup_matches(exact, None)
