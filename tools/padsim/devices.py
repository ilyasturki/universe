"""A recorded pad made real: over /dev/uhid (the HID drivers bind it as they would the pad on that bus), as a USB device through
FunctionFS on dummy_hcd (xpad binds it; root, in a VM), or as a bare evdev node through /dev/uinput (no driver: what the node
says is all there is)."""

import contextlib
import fcntl
import os
import select
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

from .corpus import EV_ABS, EV_KEY, EV_SYN, STICKS, TRIGGERS, source
from .protocols import GADGETS, PROTOCOLS, State

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "modules" / "pads" / "bin"))
import _uhid


class Pad:
    """What every kind of device answers to: press a slot, move a stick or trigger, take the device away and bring it back."""

    def __init__(self, pad, name=None, uniq=""):
        self.pad = pad
        self.name = name or pad.evdev["name"]
        self.uniq = uniq or ("02:00:00:00:00:01" if pad.bus == "bluetooth" else "")
        self.state = State()
        self.lock = threading.Lock()
        self.outputs = []
        self.alive = False

    def press(self, slot, down=True):
        with self.lock:
            if down:
                self.state.pressed.add(slot)
            else:
                self.state.pressed.discard(slot)
        self.changed({slot})

    def move(self, role, value):
        with self.lock:
            self.state.axes[role] = value
        self.changed(set())

    def snapshot(self):
        with self.lock:
            return State(set(self.state.pressed), dict(self.state.axes))

    def changed(self, slots):
        raise NotImplementedError

    def create(self):
        raise NotImplementedError

    def destroy(self):
        raise NotImplementedError

    def close(self):
        if self.alive:
            self.destroy()


class UhidPad(Pad):
    def __init__(self, pad, name=None, uniq=""):
        super().__init__(pad, name, uniq)
        self.wake_r, self.wake_w = os.pipe()
        self.create()

    def create(self):
        self.protocol = PROTOCOLS[self.pad.hid["protocol"]](self.pad, self.uniq)
        bus = _uhid.BUS_BLUETOOTH if self.pad.bus == "bluetooth" else _uhid.BUS_USB
        descriptor = bytes.fromhex(self.pad.hid["descriptor"])
        self.device = _uhid.Device(self.name, self.uniq, descriptor, bus, self.pad.vendor, self.pad.product, self.pad.version)
        self.alive = True
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()

    def changed(self, slots):
        os.write(self.wake_w, b"!")

    def serve(self):
        due = 0.0
        while self.alive:
            ready, _, _ = select.select([self.device.fd, self.wake_r], [], [], max(0.0, due - time.monotonic()))
            woken = self.wake_r in ready
            if woken:
                os.read(self.wake_r, 4096)
            if self.device.fd in ready:
                with contextlib.suppress(OSError):
                    self.answer(*self.device.event())
            if self.alive and (woken or time.monotonic() >= due):
                with contextlib.suppress(OSError):
                    self.device.input(self.protocol.report(self.snapshot()))
                due = time.monotonic() + self.protocol.period

    def answer(self, kind, fields):
        if kind == _uhid.UHID_GET_REPORT:
            request, number, _ = fields
            self.device.reply_get(request, self.protocol.feature(number))
        elif kind == _uhid.UHID_SET_REPORT:
            self.outputs.append(fields[3])
            self.device.reply_set(fields[0])
        elif kind == _uhid.UHID_OUTPUT:
            self.outputs.append(fields[0])
            for reply in self.protocol.output(fields[0]):
                self.device.input(reply)

    def destroy(self):
        self.alive = False
        os.write(self.wake_w, b"!")
        self.thread.join(2)
        self.device.close()


UI_SET_EVBIT, UI_SET_KEYBIT, UI_SET_ABSBIT = 0x40045564, 0x40045565, 0x40045567
UI_DEV_SETUP, UI_ABS_SETUP, UI_DEV_CREATE, UI_DEV_DESTROY = 0x405C5503, 0x401C5504, 0x5501, 0x5502
BUS = {"usb": 0x03, "bluetooth": 0x05}


class UinputPad(Pad):
    def __init__(self, pad, name=None, uniq=""):
        super().__init__(pad, name, uniq)
        self.ranges = pad.ranges()
        self.create()

    def create(self):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK | os.O_CLOEXEC)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_KEY)
        for code in self.pad.key_codes():
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, code)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_ABS)
        for code, (lo, hi, fuzz, flat) in self.ranges.items():
            fcntl.ioctl(self.fd, UI_SET_ABSBIT, code)
            fcntl.ioctl(self.fd, UI_ABS_SETUP, struct.pack("<H2xiiiiii", code, self.rest(code), lo, hi, fuzz, flat, 0))
        setup = struct.pack("<HHHH80sI", BUS.get(self.pad.bus, 0x03), self.pad.vendor, self.pad.product, self.pad.version, self.name.encode()[:79], 0)
        fcntl.ioctl(self.fd, UI_DEV_SETUP, setup)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        self.alive = True

    def rest(self, code):
        lo, hi = self.ranges[code][:2]
        trigger = any(self.pad.axis(t) and self.pad.axis(t)[0] == code for t in TRIGGERS)
        return lo if trigger else (lo + hi) // 2

    def emit(self, kind, code, value):
        os.write(self.fd, struct.pack("<qqHHi", 0, 0, kind, code, value))

    def changed(self, slots):
        state = self.snapshot()
        for slot in slots:
            kind, code, *sign = source(self.pad.buttons[slot])
            down = slot in state.pressed
            if kind == "key":
                self.emit(EV_KEY, code, int(down))
            else:
                lo, hi = self.ranges[code][:2]
                self.emit(EV_ABS, code, (hi if sign[0] else lo) if down else self.rest(code))
        for role in STICKS + TRIGGERS:
            axis = self.pad.axis(role)
            if axis is None or role not in state.axes:
                continue
            code, backwards = axis
            lo, hi = self.ranges[code][:2]
            share = state.axes[role] if role in TRIGGERS else ((-1 if backwards else 1) * state.axes[role] + 1) / 2
            self.emit(EV_ABS, code, round(lo + share * (hi - lo)))
        self.emit(EV_SYN, 0, 0)

    def destroy(self):
        self.alive = False
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)


FFS_DESCRIPTORS_V2, FFS_STRINGS = 3, 2
FFS_HAS_FS, FFS_HAS_HS = 1, 2
FFS_SETUP = 4
CONFIGFS = Path("/sys/kernel/config/usb_gadget")


def _interface(cls, sub, proto):
    return bytes([9, 4, 0, 0, 2, cls, sub, proto, 1])


def _endpoint(address, size, interval):
    return bytes([7, 5, address]) + struct.pack("<BHB", 3, size, interval)


class GadgetPad(Pad):
    """An Xbox pad on dummy_hcd's bus: configfs makes the device, FunctionFS hands its interface's endpoints here."""

    counter = 0

    def __init__(self, pad, name=None, uniq=""):
        super().__init__(pad, name, uniq)
        GadgetPad.counter += 1
        self.tag = f"pad{os.getpid()}x{GadgetPad.counter}"
        self.encoder = GADGETS[pad.usb["protocol"]](pad)
        self.wake = threading.Condition()
        self.create()

    def write(self, path, value):
        Path(path).write_text(value)

    def create(self):
        usb = self.pad.usb
        root = CONFIGFS / self.tag
        root.mkdir(parents=True)
        self.write(root / "idVendor", f"0x{self.pad.vendor:04x}")
        self.write(root / "idProduct", f"0x{self.pad.product:04x}")
        self.write(root / "bcdDevice", f"0x{usb['version']}")
        self.write(root / "bcdUSB", "0x0200")
        cls, sub, proto = usb["class"]
        self.write(root / "bDeviceClass", str(cls))
        self.write(root / "bDeviceSubClass", str(sub))
        self.write(root / "bDeviceProtocol", str(proto))
        (root / "strings/0x409").mkdir(parents=True)
        self.write(root / "strings/0x409/manufacturer", usb.get("manufacturer", "Microsoft"))
        self.write(root / "strings/0x409/product", usb.get("product", "Controller"))
        self.write(root / "strings/0x409/serialnumber", self.tag)
        config = root / "configs/c.1"
        (config / "strings/0x409").mkdir(parents=True)
        self.write(config / "strings/0x409/configuration", "pad")
        self.write(config / "MaxPower", "500")
        function = root / f"functions/ffs.{self.tag}"
        function.mkdir(parents=True)
        (config / f"ffs.{self.tag}").symlink_to(function)
        self.mount = Path(f"/dev/ffs-{self.tag}")
        self.mount.mkdir(exist_ok=True)
        subprocess.run(["mount", "-t", "functionfs", self.tag, str(self.mount)], check=True)
        self.ep0 = os.open(self.mount / "ep0", os.O_RDWR)
        size, out_size = usb.get("packet", 64), usb.get("packet", 64)
        descriptors = _interface(cls, sub, proto) + _endpoint(0x81, size, 4) + _endpoint(0x02, out_size, 8)
        body = struct.pack("<II", 3, 3) + descriptors + descriptors
        os.write(self.ep0, struct.pack("<III", FFS_DESCRIPTORS_V2, 12 + len(body), FFS_HAS_FS | FFS_HAS_HS) + body)
        strings = struct.pack("<H", 0x0409) + b"pad\0"
        os.write(self.ep0, struct.pack("<IIII", FFS_STRINGS, 16 + len(strings), 1, 1) + strings)
        self.ep_in = os.open(self.mount / "ep1", os.O_RDWR)
        self.ep_out = os.open(self.mount / "ep2", os.O_RDWR)
        self.alive = True
        self.threads = [threading.Thread(target=t, daemon=True) for t in (self.control, self.reader, self.writer)]
        for t in self.threads:
            t.start()
        self.write(root / "UDC", usb.get("udc") or min(os.listdir("/sys/class/udc")))

    def control(self):
        while self.alive:
            try:
                event = os.read(self.ep0, 12)
            except OSError:
                return
            if len(event) < 12 or event[8] != FFS_SETUP:
                continue
            request_type, _request, _value, _index, length = struct.unpack_from("<BBHHH", event)
            with contextlib.suppress(OSError):
                if request_type & 0x80:
                    os.write(self.ep0, bytes(length))
                else:
                    os.read(self.ep0, length)

    def reader(self):
        while self.alive:
            try:
                data = os.read(self.ep_out, 64)
            except OSError:
                if not self.alive:
                    return
                time.sleep(0.05)
                continue
            self.outputs.append(data)

    def writer(self):
        while self.alive:
            for packet in self.encoder.packets(self.snapshot()):
                try:
                    os.write(self.ep_in, packet)
                except OSError:
                    time.sleep(0.05)
            with self.wake:
                self.wake.wait(0.1)

    def changed(self, slots):
        with self.wake:
            self.wake.notify_all()

    def destroy(self):
        self.alive = False
        root = CONFIGFS / self.tag
        with contextlib.suppress(OSError):
            self.write(root / "UDC", "\n")
        self.changed(set())
        for fd in (self.ep_in, self.ep_out, self.ep0):
            with contextlib.suppress(OSError):
                os.close(fd)
        for t in self.threads:
            t.join(2)
        subprocess.run(["umount", str(self.mount)], check=False)
        with contextlib.suppress(OSError):
            self.mount.rmdir()
        for path in (
            root / f"configs/c.1/ffs.{self.tag}",
            root / "configs/c.1/strings/0x409",
            root / "configs/c.1",
            root / f"functions/ffs.{self.tag}",
            root / "strings/0x409",
            root,
        ):
            with contextlib.suppress(OSError):
                path.unlink() if path.is_symlink() else path.rmdir()


def start(pad, via="kernel", name=None, uniq=""):
    """`kernel`: the pad on the bus its entry records, through the driver that reads it there; `uinput`: its evdev node alone."""
    if via == "uinput":
        return UinputPad(pad, name, uniq)
    if pad.hid:
        return UhidPad(pad, name, uniq)
    if pad.usb:
        return GadgetPad(pad, name, uniq)
    raise ValueError(f"{pad.name}: tests/pads has no device to replay it as")
