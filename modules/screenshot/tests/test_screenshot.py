import json
import os
import shutil
import stat
import subprocess
from pathlib import Path

import pytest

SHOT = Path(__file__).resolve().parents[1] / "bin" / "shot"


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
        bindir / "universe",
        f'''printf "%s\\n" "$@" >> "{logs}/universe.args"
case "$1" in
  nest-shot) [ "${{FAKE_NEST_OK:-true}}" = true ] || {{ echo "gamescope wrote no screenshot" >&2; exit 1; }};;
  desktop-shot) [ "${{FAKE_DESKTOP_OK:-true}}" = true ] || {{ echo "grim not on PATH" >&2; exit 1; }};;
esac
echo fake > "$2"; echo "$2"''',
    )
    _write_shim(
        bindir / "gpu-screen-recorder",
        f'''printf "%s\\n" "$@" >> "{logs}/gsr.args"
for ((i=1; i<=$#; i++)); do [ "${{!i}}" = "-o" ] && {{ j=$((i+1)); echo fake > "${{!j}}"; }}; done
exit 0''',
    )
    return {"bin": bindir, "logs": logs}


def env_for(tmp_path, fakebin, settings, **extra):
    return {
        "PATH": f"{fakebin['bin']}:{os.path.dirname(shutil.which('python3'))}",
        "HOME": str(tmp_path),
        "UNIVERSE_BIN": str(fakebin["bin"] / "universe"),
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


def universe_calls(fakebin):
    return (fakebin["logs"] / "universe.args").read_text()


def test_a_shot_lands_under_the_screenshots_dir(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}))
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path.parent == tmp_path / "screenshots" and path.suffix == ".png"
    assert path.exists() and path.stat().st_size > 0


def test_the_desktop_grabs_the_window_by_default_and_the_screen_when_asked(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {"cursor": True}))
    assert result.returncode == 0, result.stderr
    assert universe_calls(fakebin) == f"desktop-shot\n{result.stdout.strip()}\n--screen\nDP-1\n--window\n--cursor\n"
    assert not (fakebin["logs"] / "gsr.args").exists()
    result = run(env_for(tmp_path, fakebin, {"window": False}))
    assert universe_calls(fakebin).endswith(f"desktop-shot\n{result.stdout.strip()}\n--screen\nDP-1\n")


def test_a_refused_desktop_grab_falls_back_to_gsr_on_the_sessions_screen(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}, FAKE_DESKTOP_OK="false", SESSION_SCREEN="HDMI-A-1"))
    assert result.returncode == 0, result.stderr
    assert "grim not on PATH" in result.stderr
    assert gsr_flag(fakebin, "-o") == [result.stdout.strip()] and gsr_flag(fakebin, "-w") == ["HDMI-A-1"]


def test_inside_gamescope_its_own_shot_is_taken_and_the_desktop_left_alone(tmp_path, fakebin):
    nested = {"GAMESCOPE_WAYLAND_DISPLAY": "gamescope-0"}
    result = run(env_for(tmp_path, fakebin, {}, **nested))
    assert result.returncode == 0, result.stderr
    assert universe_calls(fakebin) == f"nest-shot\n{result.stdout.strip()}\n"
    result = run(env_for(tmp_path, fakebin, {"window": False}, **nested))
    assert universe_calls(fakebin).endswith(f"nest-shot\n{result.stdout.strip()}\n--overlays\n"), "every layer: the HUD too"


def test_a_failed_gamescope_shot_falls_back_to_gsr(tmp_path, fakebin):
    result = run(env_for(tmp_path, fakebin, {}, GAMESCOPE_WAYLAND_DISPLAY="gamescope-0", FAKE_NEST_OK="false"))
    assert result.returncode == 0, result.stderr
    assert "gamescope wrote no screenshot" in result.stderr and gsr_flag(fakebin, "-o") == [result.stdout.strip()]


def test_no_desktop_shot_and_no_gsr_fails_saying_so(tmp_path, fakebin):
    (fakebin["bin"] / "gpu-screen-recorder").unlink()
    result = run(env_for(tmp_path, fakebin, {}, FAKE_DESKTOP_OK="false"))
    assert result.returncode == 1 and result.stdout == ""
    assert "no gpu-screen-recorder" in result.stderr
