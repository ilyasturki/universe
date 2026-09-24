import json
import os
import shutil
import stat
import subprocess
from pathlib import Path

import pytest

SHOT = Path(__file__).resolve().parents[1] / "bin" / "shot"
UUID = "universe@ilyasturki.github.io"


def _write_shim(path, body):
    path.write_text(f"#!{shutil.which('bash')}\n{body}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)


@pytest.fixture
def fakebin(tmp_path):
    bindir = tmp_path / "fakebin"
    bindir.mkdir()
    logs = tmp_path / "calls"
    logs.mkdir()
    _write_shim(
        bindir / "busctl",
        f'''printf "%s\\n" "$@" >> "{logs}/busctl.args"
case "$*" in
  *NameHasOwner*|*EnableExtension*) echo "{{\\"type\\":\\"b\\",\\"data\\":[${{FAKE_NAME_OWNED:-false}}]}}"; exit 0;;
  *" Screenshot "*) [ "${{FAKE_SHOT_OK:-true}}" = true ] && echo fake > "$9"; echo "{{\\"type\\":\\"b\\",\\"data\\":[${{FAKE_SHOT_OK:-true}}]}}"; exit 0;;
esac
exit 0''',
    )
    _write_shim(
        bindir / "universe",
        f'''printf "%s\\n" "$@" >> "{logs}/universe.args"
[ "${{FAKE_NEST_OK:-true}}" = true ] || {{ echo "gamescope wrote no screenshot" >&2; exit 1; }}
echo fake > "$2"; echo "$2"''',
    )
    _write_shim(
        bindir / "gpu-screen-recorder",
        f'''printf "%s\\n" "$@" >> "{logs}/gsr.args"
for ((i=1; i<=$#; i++)); do [ "${{!i}}" = "-o" ] && {{ j=$((i+1)); echo fake > "${{!j}}"; }}; done
exit 0''',
    )
    return {"bin": bindir, "logs": logs}


def env_for(tmp_path, fakebin, settings, extension=False, **extra):
    home = tmp_path / "home"
    home.mkdir(exist_ok=True)
    if extension:
        (home / ".local/share/gnome-shell/extensions" / UUID).mkdir(parents=True, exist_ok=True)
    return {
        "PATH": f"{fakebin['bin']}:{os.path.dirname(shutil.which('python3'))}",
        "HOME": str(home),
        "XDG_DATA_DIRS": str(tmp_path / "datadirs"),
        "XDG_CURRENT_DESKTOP": "GNOME",
        "SCREENSHOTS_DIR": str(tmp_path / "screenshots"),
        "SESSION_SCREEN": "DP-1",
        "MODULE_SETTINGS_JSON": json.dumps(settings),
        **extra,
    }


def run(env):
    return subprocess.run([str(SHOT)], env=env, capture_output=True, text=True, timeout=30, check=False)


def gsr_flag(fakebin, flag):
    args = (fakebin["logs"] / "gsr.args").read_text().splitlines()
    return [args[i + 1] for i, a in enumerate(args) if a == flag]


def test_a_shot_lands_under_the_screenshots_dir(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "screenshots" and path.suffix == ".png"
    assert path.exists() and path.stat().st_size > 0


def test_the_shell_grabs_the_window_with_the_cursor_when_asked(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {"window": True, "cursor": True}, extension=True, FAKE_NAME_OWNED="true"))
    assert result.returncode == 0, result.stderr
    assert f"Screenshot\nsbb\n{result.stdout.strip()}\ntrue\ntrue\n" in (fakebin["logs"] / "busctl.args").read_text()
    assert not (fakebin["logs"] / "gsr.args").exists()


def test_the_shell_grabs_the_window_by_default_and_every_monitor_when_asked(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}, extension=True, FAKE_NAME_OWNED="true"))
    assert result.returncode == 0, result.stderr
    assert f"\nsbb\n{result.stdout.strip()}\ntrue\nfalse\n" in (fakebin["logs"] / "busctl.args").read_text()
    result = run(env_for(tmp_path, fakebin, {"window": False}, extension=True, FAKE_NAME_OWNED="true"))
    assert f"\nsbb\n{result.stdout.strip()}\nfalse\nfalse\n" in (fakebin["logs"] / "busctl.args").read_text()


def test_a_refused_shell_grab_falls_back_to_gsr_on_the_sessions_screen(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}, extension=True, FAKE_NAME_OWNED="true", FAKE_SHOT_OK="false", SESSION_SCREEN="HDMI-A-1"))
    assert result.returncode == 0, result.stderr
    assert "the shell refused" in result.stderr
    assert gsr_flag(fakebin, "-o") == [result.stdout.strip()] and gsr_flag(fakebin, "-w") == ["HDMI-A-1"]


def test_inside_gamescope_its_own_shot_is_taken_and_the_shell_left_alone(tmp_path, fakebin):
    nested = {"GAMESCOPE_WAYLAND_DISPLAY": "gamescope-0", "UNIVERSE_BIN": str(fakebin["bin"] / "universe")}
    result = run(env_for(tmp_path, fakebin, {}, extension=True, FAKE_NAME_OWNED="true", **nested))
    assert result.returncode == 0, result.stderr
    assert (fakebin["logs"] / "universe.args").read_text() == f"nest-shot\n{result.stdout.strip()}\n"
    assert not (fakebin["logs"] / "busctl.args").exists()
    result = run(env_for(tmp_path, fakebin, {"window": False}, **nested))
    assert (fakebin["logs"] / "universe.args").read_text().endswith(f"nest-shot\n{result.stdout.strip()}\n--overlays\n"), "every layer: the HUD too"


def test_a_failed_gamescope_shot_falls_back_to_gsr(tmp_path, fakebin):
    nested = {"GAMESCOPE_WAYLAND_DISPLAY": "gamescope-0", "UNIVERSE_BIN": str(fakebin["bin"] / "universe"), "FAKE_NEST_OK": "false"}
    result = run(env_for(tmp_path, fakebin, {}, **nested))
    assert result.returncode == 0, result.stderr
    assert "gamescope wrote no screenshot" in result.stderr and gsr_flag(fakebin, "-o") == [result.stdout.strip()]


def test_no_shell_and_no_gsr_fails_saying_so(tmp_path, fakebin):
    (fakebin["bin"] / "gpu-screen-recorder").unlink()
    result = run(env_for(tmp_path, fakebin, {}))
    assert result.returncode == 1 and result.stdout == ""
    assert "no gpu-screen-recorder" in result.stderr
