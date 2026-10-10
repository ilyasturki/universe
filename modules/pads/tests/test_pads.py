import importlib.machinery
import importlib.util
import json
import math
import os
import shutil
import signal
import stat
import struct
import sys
from pathlib import Path

import pytest

BIN_DIR = Path(__file__).resolve().parents[1] / "bin"
sys.path.insert(0, str(BIN_DIR))
import _pads  # noqa: E402
from _pads import GUIDE, MISC1, NEUTRAL, SOUTH, PadState, pack_report, parse_rumble  # noqa: E402


def _script(name):
    loader = importlib.machinery.SourceFileLoader(f"pads_{name}", str(BIN_DIR / name))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


def sdl_decode(report):
    """What SDL_hidapi_ps5.c reads out of a third-party pad's USB report, uncalibrated."""
    p = report[1:]
    gyro = [v * 64 / 1024 * math.pi / 180 for v in struct.unpack_from("<hhh", p, 15)]
    accel = [v / 8192 * 9.80665 for v in struct.unpack_from("<hhh", p, 21)]
    tx = p[32] | ((p[33] & 0x0F) << 8)
    ty = (p[33] >> 4) | (p[34] << 4)
    return {
        "sticks": [p[i] * 257 - 32768 for i in range(4)],
        "triggers": [p[4] * 257 - 32768, p[5] * 257 - 32768],
        "hat": p[7] & 0x0F,
        "face": p[7] >> 4,
        "buttons1": p[8],
        "buttons2": p[9],
        "gyro": gyro,
        "accel": accel,
        "tick": struct.unpack_from("<H", p, 27)[0],
        "battery": p[29],
        "touch": None if p[31] & 0x80 else (tx / 1920, ty / 1070),
    }


def test_a_neutral_pad_is_centred_at_rest_and_reports_no_battery():
    report = pack_report(NEUTRAL, 1, 0)
    got = sdl_decode(report)
    assert len(report) == 64 and report[0] == 0x01
    assert all(abs(v) <= 128 for v in got["sticks"])
    assert got["triggers"] == [-32768, -32768]
    assert got["hat"] == 8 and got["face"] == 0 and got["buttons1"] == 0 and got["buttons2"] == 0
    assert got["gyro"] == [0, 0, 0]
    assert got["accel"][1] == pytest.approx(9.80665, abs=0.01)
    assert got["battery"] == 0x0C
    assert got["touch"] is None


def test_sticks_triggers_and_motion_come_out_as_sdl_read_them_in():
    state = PadState(axes=(-32768, 32767, 1000, -1000, 32767, 16384), gyro=(1.5, -0.25, 3.0), accel=(2.0, -9.8, 4.5))
    got = sdl_decode(pack_report(state, 7, 70000))
    assert got["sticks"] == pytest.approx([-32768, 32767, 1000, -1000], abs=256)
    assert got["triggers"][0] == 32767
    assert got["gyro"] == pytest.approx([1.5, -0.25, 3.0], abs=0.002)
    assert got["accel"] == pytest.approx([2.0, -9.8, 4.5], abs=0.002)
    assert got["tick"] == 70000 & 0xFFFF


def test_buttons_land_on_the_dualsense_bits_and_home_stays_with_universe():
    state = PadState(buttons=frozenset({SOUTH, _pads.DPAD_UP, _pads.DPAD_RIGHT, _pads.LEFT_SHOULDER, _pads.START, GUIDE, MISC1, _pads.TOUCHPAD}))
    kept = sdl_decode(pack_report(state, 1, 0, guide=False))
    assert kept["face"] == 0x2
    assert kept["hat"] == 1
    assert kept["buttons1"] == 0x01 | 0x20
    assert kept["buttons2"] == 0x02 | 0x04
    assert sdl_decode(pack_report(state, 1, 0, guide=True))["buttons2"] == 0x01 | 0x02 | 0x04


def test_a_finger_and_the_battery_are_encoded_as_the_driver_reads_them():
    state = PadState(touches=((0.5, 0.25), None), power=(_pads.POWER_CHARGING, 55))
    got = sdl_decode(pack_report(state, 1, 0))
    assert got["touch"] == pytest.approx((0.5, 0.25), abs=0.001)
    assert got["battery"] == 0x15
    assert sdl_decode(pack_report(PadState(power=(_pads.POWER_CHARGED, 100)), 1, 0))["battery"] == 0x2A


def test_rumble_is_read_back_at_the_strength_the_emulator_asked():
    # Measured: SDL_RumbleGamepad(0x8000, 0xC000) on the virtual pad arrives as flags 03, right 96, left 64.
    report = bytes([0x02, 0x03, 0x00, 96, 64]).ljust(48, b"\0")
    low, high = parse_rumble(report)
    assert low == pytest.approx(0x8000, abs=512)
    assert high == pytest.approx(0xC000, abs=512)
    assert parse_rumble(bytes([0x02, 0x02, 0, 0, 0]).ljust(48, b"\0")) == (0, 0)
    assert parse_rumble(bytes([0x05, 0x03, 0, 96, 64])) is None


def _shim(path, body):
    path.write_text(f"#!{shutil.which('bash')}\n{body}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


@pytest.fixture
def hook(tmp_path, monkeypatch):
    bindir = tmp_path / "bin"
    bindir.mkdir()
    runtime = tmp_path / "run"
    runtime.mkdir()
    calls = tmp_path / "calls"
    state = runtime / "universe" / "pads-s1.json"
    state.parent.mkdir()
    _shim(
        bindir / "systemd-run",
        f'''printf "%s\\n" "$@" > "{calls}"
[ -n "$FAKE_NO_READY" ] || {{ echo '{{"ready": true, "players": [{{"name": "Universe Pad 1", "pad": "Xbox"}}]}}' > "{state}"; }}''',
    )
    _shim(bindir / "systemctl", 'case "$*" in *is-active*) echo "${FAKE_UNIT_STATE:-active}";; esac')
    uhid = tmp_path / "uhid"
    uhid.write_text("")
    env_file = tmp_path / "env"
    env_file.write_text("")
    monkeypatch.setenv("PATH", f"{bindir}:{Path(shutil.which('bash')).parent}")
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(runtime))
    monkeypatch.setenv("SESSION_ID", "s1")
    monkeypatch.setenv("SESSION_UNIT", "universe-game-s1.service")
    monkeypatch.setenv("UNIVERSE_ENV_FILE", str(env_file))
    monkeypatch.setenv("MODULE_SETTINGS_JSON", json.dumps({"enabled": True, "guide": False}))
    monkeypatch.setenv("SDL_JOYSTICK_HIDAPI_XBOX", "0")
    monkeypatch.setattr(_pads, "UHID", str(uhid))
    pre = _script("pre")
    monkeypatch.setattr(pre, "READY_WAIT_S", 0.5)

    def run(effective):
        monkeypatch.setenv("UNIVERSE_GAME_JSON", json.dumps({"effective": effective}))
        assert pre.main() == 0
        return env_file.read_text(), calls.read_text().splitlines() if calls.exists() else None

    return run, monkeypatch


EDEN = {"runner": "eden", "runner_kind": "emulator"}


def test_an_emulator_gets_the_virtual_pads_and_the_forwarder_the_users_sdl_env(hook):
    run, _ = hook
    env, args = run(EDEN)
    assert "SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT=0x0079/0x5550," in env
    assert "SDL_JOYSTICK_HIDAPI_PS5=1\n" in env
    assert "--unit=universe-pads-s1.service" in args
    assert "--setenv=SDL_JOYSTICK_HIDAPI_XBOX=0" in args
    assert "--setenv=PADS_GUIDE=0" in args
    assert args[-1].endswith("/forward")


def test_a_proton_game_keeps_its_pads(hook):
    run, _ = hook
    env, args = run({"runner": "proton", "runner_kind": "proton"})
    assert env == ""
    assert args is None


def test_no_uhid_access_leaves_the_pads_as_they_are(hook, tmp_path):
    run, monkeypatch = hook
    monkeypatch.setattr(_pads, "UHID", str(tmp_path / "absent"))
    env, args = run(EDEN)
    assert env == ""
    assert args is None


def test_a_forwarder_that_never_comes_up_does_not_hide_the_pads(hook):
    run, monkeypatch = hook
    monkeypatch.setenv("FAKE_NO_READY", "1")
    monkeypatch.setenv("FAKE_UNIT_STATE", "failed")
    env, args = run(EDEN)
    assert env == ""
    assert args is not None


class FakeSdl:
    """The forwarder's SDL, as far as a re-init goes: what was asked of it."""

    def __init__(self):
        self.calls = []

    def SDL_QuitSubSystem(self, flags):
        self.calls.append(("quit", flags))

    def SDL_InitSubSystem(self, flags):
        self.calls.append(("init", flags))
        return True

    def SDL_SetEventEnabled(self, kind, on):
        pass

    def SDL_OpenGamepad(self, instance_id):
        return instance_id


class FakeGamepad:
    """A physical pad SDL opened: instance ids 1, 3 are an 8BitDo, 2, 4 a DualSense, each listed again after a re-init."""

    def __init__(self, _sdl, _handle, instance_id):
        self.id, self.gyro = instance_id, False
        self.vendor, self.product, self.name = ((0x2DC8, 0x6009, "8BitDo Pro 3"), (0x054C, 0x0CE6, "DualSense"))[(instance_id + 1) % 2]

    def close(self):
        pass


def uhid_events(data):
    """The (type, name, uniq, product) of each event written to /dev/uhid, in order."""
    out, at = [], 0
    while at < len(data):
        (kind,) = struct.unpack_from("<I", data, at)
        if kind == _pads.UHID_CREATE2:
            _, name, _phys, uniq, rd_size, _bus, _vendor, product, _version, _country = struct.unpack_from("<I128s64s64sHHIIII", data, at)
            out.append(("create", name.rstrip(b"\0").decode(), uniq.rstrip(b"\0").decode(), product))
            at += struct.calcsize("<I128s64s64sHHIIII") + rd_size
        else:
            out.append(("destroy" if kind == _pads.UHID_DESTROY else kind, "", "", 0))
            at += 4
    return out


def test_a_hangup_makes_every_virtual_pad_again_under_its_own_name_and_uniq(tmp_path, monkeypatch):
    uhid = tmp_path / "uhid"
    os.mkfifo(uhid)
    reader = os.open(uhid, os.O_RDONLY | os.O_NONBLOCK)
    monkeypatch.setattr(_pads, "UHID", str(uhid))
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    forward = _script("forward")
    monkeypatch.setattr(forward._sdl, "Gamepad", FakeGamepad)
    sdl = FakeSdl()
    fw = forward.Forwarder(sdl, "s1", False)
    fw.add_slot()
    fw.add_slot()
    fw.on_added(1)
    fw.on_added(2)
    created = uhid_events(os.read(reader, 65536))
    previous = signal.signal(signal.SIGHUP, fw.on_signal)
    try:
        os.kill(os.getpid(), signal.SIGHUP)
    finally:
        signal.signal(signal.SIGHUP, previous)
    assert fw.reconnecting, "the hook's signal reaches the forwarder"
    fw.recreate()
    again = uhid_events(os.read(reader, 65536))
    assert [e[0] for e in again] == ["destroy", "destroy", "create", "create"]
    assert again[2:] == created, "each player's pad back under the same name, uniq and product"
    assert sdl.calls == [("quit", forward._sdl.INIT_GAMEPAD), ("init", forward._sdl.INIT_GAMEPAD)], "the physical pads are listed afresh"
    fw.on_added(4)
    fw.on_added(3)
    assert {p: g.name for p, g in fw.held.items()} == {0: "8BitDo Pro 3", 1: "DualSense"}, "each pad back on its own player, whatever order SDL lists them in"
    fw.close()
    os.close(reader)
