"""Each recorded pad made real, then read the way Universe reads it: the driver that binds it, `controller ls`, the watcher's event for
every button (HOME as guide, a hold firing its macro), the pad going away and coming back, the launcher confirming with the pad's A,
SDL3's view of it, and the virtual pads' forwarder carrying a press out and a rumble back. `--via uinput` (`just test-pads`, on the
desktop: the evdev side alone) or `--via kernel` (the VM check: uhid and USB gadgets the real drivers bind)."""

import argparse
import contextlib
import json
import os
import queue
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

from . import corpus, devices

REPO = Path(__file__).resolve().parents[2]
CONFIRM = {"A", "Cross"}


class Failed(Exception):
    pass


class Lines:
    """A process's output lines, read as they come."""

    def __init__(self, proc):
        self.proc = proc
        self.queue = queue.Queue()
        self.seen = []
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.proc.stdout:
            self.queue.put(line.rstrip("\n"))
        self.queue.put(None)

    def wait(self, match, timeout, what):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                line = self.queue.get(timeout=max(0.0, deadline - time.monotonic()))
            except queue.Empty:
                break
            if line is None:
                raise Failed(f"{what}: the process ended ({self.proc.poll()}); last lines: {self.seen[-5:]}")
            self.seen.append(line)
            found = match(line)
            if found:
                return found
        raise Failed(f"{what}: nothing within {timeout:g} s; last lines: {self.seen[-5:]}")


def events(match):
    def parse(line):
        try:
            event = json.loads(line)
        except ValueError:
            return None
        return event if match(event) else None

    return parse


class Profile:
    """A profile, runtime directory and lock of its own: the desktop's launcher and its watcher are left alone."""

    def __init__(self, universe, modules):
        self.dir = Path(tempfile.mkdtemp(prefix="pad-check-"))
        self.universe = universe
        runtime = self.dir / "run"
        runtime.mkdir(mode=0o700)
        self.env = {
            **os.environ,
            "XDG_RUNTIME_DIR": str(runtime),
            "UNIVERSE_BIN": universe,
            "UNIVERSE_MODULES_PATH": modules,
            "UNIVERSE_SOURCES_PATH": str(self.dir / "sources"),
            "PIPEWIRE_REMOTE": "/nonexistent",
            "RUST_LOG": "warn",
        }
        for kind in ("DATA", "CONFIG", "STATE", "CACHE"):
            path = self.dir / kind.lower()
            path.mkdir()
            self.env[f"UNIVERSE_{kind}_HOME"] = str(path)
        (self.dir / "config/config.toml").write_text('schema = 1\n[modules]\nenabled = ["controls"]\n[sources]\nenabled = []\n')
        self.run("controller", "bind", "*", "start", "hold", "mangohud")

    def run(self, *args, check=True):
        out = subprocess.run([self.universe, *args], env=self.env, capture_output=True, text=True, timeout=60, check=False)
        if check and out.returncode != 0:
            raise Failed(f"universe {' '.join(args)}: {out.stderr.strip()[-300:]}")
        return out.stdout

    def json(self, *args):
        return json.loads(self.run("--json", *args))

    def spawn(self, argv, **env):
        return subprocess.Popen(argv, env={**self.env, **env}, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    def close(self):
        shutil.rmtree(self.dir, ignore_errors=True)


def stop(proc):
    if proc.poll() is None:
        proc.send_signal(signal.SIGTERM)
        try:
            proc.wait(5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def sysfs_device(event_id):
    return os.path.realpath(f"/sys/class/input/{event_id}/device/device")


class Check:
    def __init__(self, pad, via, profile, ui, record):
        self.pad = pad
        self.via = via
        self.profile = profile
        self.ui = ui
        self.record = record
        self.name = None if via == "kernel" else f"{pad.evdev['name']} (pad check {os.getpid()})"[:79]
        self.failures = []
        self.recorded = {}

    def step(self, what, fn):
        try:
            fn()
        except Failed as e:
            self.failures.append(f"{what}: {e}")
        except OSError as e:
            self.failures.append(f"{what}: {e}")

    def find(self, timeout=10):
        """The pad as `controller ls` lists it, once it is there."""
        deadline = time.monotonic() + timeout
        while True:
            listed = self.profile.json("controller", "ls")["devices"]
            mine = [d for d in listed if (d["vendor"], d["product"]) == (self.pad.vendor, self.pad.product) and (self.name is None or d["name"] == self.name)]
            if mine or time.monotonic() > deadline:
                return mine
            time.sleep(0.3)

    def run(self):
        start = time.monotonic()
        if self.pad.family == "switch2-pro":
            self.step("doctor", self.doctor)
            return time.monotonic() - start
        self.device = devices.start(self.pad, self.via, self.name)
        try:
            self.step("listed", self.listed)
            if self.failures:
                return time.monotonic() - start
            if self.via == "kernel":
                self.step("driver", self.driver)
                self.step("sdl", self.sdl)
                if self.record:
                    self.step("dump", self.dump)
            self.step("watcher", self.watcher)
            if self.ui:
                self.step("launcher", self.launcher)
            if self.via == "kernel":
                self.step("forwarder", self.forwarder)
        finally:
            self.device.close()
        return time.monotonic() - start

    def listed(self):
        mine = self.find()
        if len(mine) != 1:
            raise Failed(f"{len(mine)} devices for it")
        self.listing = mine[0]
        if self.listing["family"] != self.pad.family:
            raise Failed(f"family {self.listing['family']}, not {self.pad.family}")
        wrong = {s: (self.listing["slots"].get(s) or {}).get("code") for s, code in self.pad.buttons.items()}
        wrong = {s: got for s, got in wrong.items() if got != self.pad.buttons[s]}
        if wrong:
            raise Failed(f"slots read otherwise: {wrong}")

    def driver(self):
        link = Path(sysfs_device(self.listing["id"])) / "driver"
        bound = os.path.basename(os.path.realpath(link)) if link.exists() else ""
        if self.pad.driver and bound != self.pad.driver:
            raise Failed(f"bound by {bound or 'nothing'}, not {self.pad.driver}")

    def sdl(self):
        hook = Path(self.profile.env["UNIVERSE_MODULES_PATH"]) / "controls" / "bin" / "pads"
        out = subprocess.run([str(hook)], env=self.profile.env, capture_output=True, text=True, timeout=30, check=False)
        lines = [json.loads(line) for line in out.stdout.splitlines() if line.startswith("{")]
        mine = [line for line in lines if line["device"] == sysfs_device(self.listing["id"])] or [
            line for line in lines if (line["vendor"], line["product"]) == (self.pad.vendor, self.pad.product)
        ]
        if not mine:
            raise Failed(f"SDL3 lists no such pad: {out.stderr.strip()[-200:]}")
        view = mine[0]["sdl"]
        self.recorded["sdl"] = view
        if self.pad.sdl and not (self.pad.hid or {}).get("synthesized") and view != self.pad.sdl:
            differ = sorted(k for k in view.keys() | self.pad.sdl.keys() if view.get(k) != self.pad.sdl.get(k))
            raise Failed(f"SDL3's view differs from tests/pads in {differ}: {json.dumps({k: view.get(k) for k in differ})}")

    def dump(self):
        self.recorded["evdev"] = self.profile.json("controller", "dump", self.listing["id"])["evdev"]

    def watcher(self):
        proc = self.profile.spawn([self.profile.universe, "--json", "controller", "watch"])
        lines = Lines(proc)
        try:
            ident = self.listing["id"]
            lines.wait(events(lambda e: e.get("event") == "device" and e.get("id") == ident), 10, "its device line")
            missing = []
            for slot in self.pad.buttons:
                for down in (True, False):
                    self.device.press(slot, down)
                    try:
                        lines.wait(
                            events(
                                lambda e, s=slot, d=down: e.get("event") == "button" and e.get("id") == ident and e.get("slot") == s and e.get("pressed") == d
                            ),
                            3,
                            slot,
                        )
                    except Failed:
                        missing.append(f"{slot} {'down' if down else 'up'}")
                        break
            if missing:
                raise Failed(f"no button event for {missing}")
            self.device.press("start")
            lines.wait(events(lambda e: e.get("event") == "macro" and e.get("trigger") == "hold" and e.get("slot") == "start"), 3, "a hold's macro")
            self.device.press("start", False)
            self.device.destroy()
            lines.wait(events(lambda e: e.get("event") == "gone" and e.get("id") == ident), 5, "gone once it went away")
            self.device.create()
            back = lines.wait(
                events(lambda e: e.get("event") == "device" and (e.get("vendor"), e.get("product")) == (self.pad.vendor, self.pad.product)),
                10,
                "back once it came back",
            )
            self.listing = {**self.listing, "id": back["id"]}
        finally:
            with contextlib.suppress(OSError):
                proc.stdin.write('{"cmd":"quit"}\n')
                proc.stdin.flush()
            stop(proc)

    def launcher(self):
        """The pad's A (Cross on a PlayStation pad) posts Return. Nothing else is pressed: B held opens the power menu."""
        families = {f["id"]: {s["id"]: s["label"] for s in f["slots"]} for f in self.profile.json("controller", "ls")["families"]}
        labels = families[self.pad.family]
        confirm = next(s for s in ("south", "east", "west", "north") if labels.get(s) in CONFIRM)
        proc = self.profile.spawn([self.ui, "--size", "640x360"], QT_QPA_PLATFORM="offscreen", UNIVERSE_UI_INPUT_LOG="1", QT_FORCE_STDERR_LOGGING="1")
        lines = Lines(proc)
        try:
            lines.wait(lambda line: "universe.gamepad: mapping:" in line and line, 30, "the launcher maps the pad")
            time.sleep(0.5)
            self.device.press(confirm)
            found = lines.wait(lambda line: "from pad" in line and " press " in line and line, 5, f"{confirm} posting a key")
            self.device.press(confirm, False)
            if "key Key_Return press" not in found:
                raise Failed(f"{confirm} ({labels[confirm]}) posted {found.split('key ', 1)[-1]}, not Return")
        finally:
            stop(proc)

    def forwarder(self):
        forward = Path(self.profile.env["UNIVERSE_MODULES_PATH"]) / "pads" / "bin" / "forward"
        session = f"padcheck{os.getpid()}"
        proc = self.profile.spawn([str(forward)], SESSION_ID=session, SESSION_UNIT=f"universe-{session}.service", PADS_GUIDE="0")
        Lines(proc)
        state = Path(self.profile.env["XDG_RUNTIME_DIR"]) / "universe" / f"pads-{session}.json"
        try:
            deadline = time.monotonic() + 15
            while True:
                with contextlib.suppress(OSError, ValueError):
                    players = json.loads(state.read_text())["players"]
                    if players and players[0].get("pad"):
                        break
                if time.monotonic() > deadline:
                    raise Failed("the forwarder never held the pad")
                time.sleep(0.2)
            node = virtual_hidraw()
            fd = os.open(node, os.O_RDWR)
            try:
                self.device.press("south")
                if not seen(fd, lambda r: r[0] == 0x01 and r[8] & 0x20, 3):
                    raise Failed("Universe Pad 1 never reported the press")
                self.device.press("south", False)
                before = len(self.device.outputs)
                os.write(fd, bytes([0x02, 0x01, 0x00, 0x80, 0x80]).ljust(48, b"\0"))
                deadline = time.monotonic() + 3
                while len(self.device.outputs) == before and time.monotonic() < deadline:
                    time.sleep(0.05)
                if len(self.device.outputs) == before:
                    raise Failed("a rumble sent to Universe Pad 1 never reached the pad")
            finally:
                os.close(fd)
        finally:
            stop(proc)

    def doctor(self):
        gadget = devices.start(self.pad, "kernel") if self.pad.usb else None
        try:
            time.sleep(2)
            checks = self.profile.json("doctor")
            line = next((c for c in checks if c.get("check") == "switch2-pro"), None)
            if gadget is not None and (line is None or line.get("ok")):
                raise Failed(f"doctor says nothing of a Switch 2 Pro on USB: {line}")
            if self.find(timeout=1):
                raise Failed("controller ls lists a pad no driver reads")
        finally:
            if gadget is not None:
                gadget.close()


def desktop_watcher():
    """Whether the desktop's own controller watcher (its launcher's, a game's) holds the pads' lock."""
    import fcntl

    lock = Path(os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}") / "universe" / "controller.lock"
    try:
        fd = os.open(lock, os.O_RDONLY)
    except OSError:
        return False
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        return True
    finally:
        os.close(fd)
    return False


def virtual_hidraw():
    for entry in Path("/sys/class/hidraw").iterdir():
        with contextlib.suppress(OSError):
            if "HID_ID=0003:00000079:00005550" in (entry / "device/uevent").read_text():
                return f"/dev/{entry.name}"
    raise Failed("no Universe Pad 1 hidraw node")


def seen(fd, match, timeout):
    import select

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], max(0.0, deadline - time.monotonic()))
        if ready and match(os.read(fd, 64)):
            return True
    return False


def main(argv=None):
    parser = argparse.ArgumentParser(prog="pad-check", description=__doc__)
    parser.add_argument("pads", nargs="*", help="tests/pads names (every one when none)")
    parser.add_argument("--via", choices=("uinput", "kernel"), default="uinput")
    parser.add_argument("--universe", default=os.environ.get("UNIVERSE_BIN") or shutil.which("universe") or "universe")
    parser.add_argument("--ui", default=os.environ.get("UNIVERSE_UI_BIN") or shutil.which("universe-ui") or "", help="universe-ui; empty skips the launcher")
    parser.add_argument("--modules", default=os.environ.get("UNIVERSE_MODULES_PATH") or str(REPO / "modules"))
    parser.add_argument("--record", action="store_true", help="print what the kernel and SDL3 show of each pad, as tests/pads keeps it")
    args = parser.parse_args(argv)
    if args.via == "uinput" and desktop_watcher():
        print("a launcher or a game's watcher reads the pads on this desktop: a press here would reach it; close it first", file=sys.stderr)
        return 2
    names = args.pads or corpus.names()
    failed, skipped = 0, 0
    for name in names:
        pad = corpus.load(name)
        if args.via == "kernel" and not (pad.hid or pad.usb or pad.family == "switch2-pro"):
            print(f"{name}: skipped, nothing to replay it as", flush=True)
            skipped += 1
            continue
        if pad.family == "switch2-pro" and args.via == "uinput":
            print(f"{name}: skipped, no driver to read it", flush=True)
            skipped += 1
            continue
        profile = Profile(args.universe, args.modules)
        try:
            check = Check(pad, args.via, profile, args.ui, args.record)
            spent = check.run()
        finally:
            profile.close()
        if args.record and check.recorded:
            print(json.dumps({"record": name, **check.recorded}), flush=True)
        status = "ok" if not check.failures else "FAILED"
        print(f"{name}: {status} ({spent:.1f} s)", flush=True)
        for failure in check.failures:
            print(f"  {failure}", flush=True)
        failed += bool(check.failures)
    print(f"{len(names) - failed - skipped} ok, {failed} failed, {skipped} skipped", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
