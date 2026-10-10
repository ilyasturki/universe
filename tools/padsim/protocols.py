"""What each kind of pad sends and answers, from the drivers that read it (Linux 7.2's hid-playstation, hid-nintendo, hid-input, xpad)
and SDL 3's HIDAPI drivers: a pad's state → its input reports; the driver's requests → the pad's replies."""

import struct
import zlib
from dataclasses import dataclass, field

from . import hid
from .corpus import AXES, STICKS, TRIGGERS, source

HAT_UP, HAT_RIGHT, HAT_DOWN, HAT_LEFT = "dpad_up", "dpad_right", "dpad_down", "dpad_left"


@dataclass
class State:
    pressed: set = field(default_factory=set)
    # stick role → -1..1 (right and down positive), trigger role → 0..1
    axes: dict = field(default_factory=dict)

    def stick(self, role):
        return self.axes.get(role, 0.0)

    def trigger(self, role):
        return self.axes.get(role, 1.0 if role in self.pressed else 0.0)


def hat_index(pressed):
    """0 up, then clockwise to 7 up-left; 8 at rest."""
    up, right, down, left = (d in pressed for d in (HAT_UP, HAT_RIGHT, HAT_DOWN, HAT_LEFT))
    table = {(1, 0, 0, 0): 0, (1, 1, 0, 0): 1, (0, 1, 0, 0): 2, (0, 1, 1, 0): 3, (0, 0, 1, 0): 4, (0, 0, 1, 1): 5, (0, 0, 0, 1): 6, (1, 0, 0, 1): 7}
    return table.get((up, right, down, left), 8)


def byte_axis(value, lo=0, hi=255):
    return max(lo, min(hi, round((value + 1) / 2 * (hi - lo) + lo)))


def crc_sign(report, seed, count):
    crc = zlib.crc32(bytes(report[:count]), zlib.crc32(bytes([seed])))
    report[count : count + 4] = struct.pack("<I", crc)


def mac_bytes(uniq):
    """A Bluetooth address, least significant byte first, as Sony's pairing reports carry it."""
    return bytes(int(b, 16) for b in reversed(uniq.split(":")))


class Protocol:
    period = 0.02

    def __init__(self, pad, uniq):
        self.pad = pad
        self.uniq = uniq
        self.bluetooth = pad.bus == "bluetooth"
        self.seq = 0

    def report(self, state):
        raise NotImplementedError

    def feature(self, number):
        """A GET_REPORT's answer, report number first; None: no such report."""
        return None

    def output(self, data):
        """An output report the driver sent: the replies it waits for, as input reports."""
        return []


SONY_FACE = {"west": 0x10, "south": 0x20, "east": 0x40, "north": 0x80}
SONY_SHOULDERS = {"lb": 0x01, "rb": 0x02, "lt": 0x04, "rt": 0x08, "select": 0x10, "start": 0x20, "ls": 0x40, "rs": 0x80}
SONY_SYSTEM = {"guide": 0x01, "touchpad": 0x02, "mute": 0x04, "fn_left": 0x10, "fn_right": 0x20, "paddle_left": 0x40, "paddle_right": 0x80}


def sony_buttons(state):
    face = hat_index(state.pressed) | sum(bit for slot, bit in SONY_FACE.items() if slot in state.pressed)
    shoulders = sum(bit for slot, bit in SONY_SHOULDERS.items() if slot in state.pressed)
    system = sum(bit for slot, bit in SONY_SYSTEM.items() if slot in state.pressed)
    return face, shoulders, system


class Sony(Protocol):
    """Feature reports from tests/pads' `hid.features`; the pairing report carries the address the pad was made with."""

    signed = ()
    pairing = ()

    def feature(self, number):
        data = (self.pad.hid.get("features") or {}).get(f"{number:02x}")
        if data is None:
            return None
        report = bytearray.fromhex(data)
        if number in self.pairing:
            report[1:7] = mac_bytes(self.uniq)
        if self.bluetooth and number in self.signed:
            crc_sign(report, 0xA3, len(report) - 4)
        return bytes(report)

    def sticks(self, state):
        return [byte_axis(state.stick(r)) for r in STICKS]

    def triggers(self, state):
        return [byte_axis(state.trigger(t) * 2 - 1) for t in TRIGGERS]


class DualSense(Sony):
    signed = (0x05, 0x09, 0x20)
    pairing = (0x09,)

    def report(self, state):
        self.seq = (self.seq + 1) & 0xFF
        body = bytearray(63)
        body[0:4] = bytes(self.sticks(state))
        body[4:6] = bytes(self.triggers(state))
        body[6] = self.seq
        body[7:10] = bytes(sony_buttons(state))
        body[32] = body[36] = 0x80  # both touch points lifted
        body[52] = 0x08  # status: on battery, 80 %
        if not self.bluetooth:
            return bytes([0x01]) + bytes(body)
        report = bytearray(78)
        report[0] = 0x31
        report[2:65] = body
        crc_sign(report, 0xA1, 74)
        return bytes(report)


class DualShock4(Sony):
    signed = (0x05,)
    pairing = (0x12, 0x81)

    def report(self, state):
        self.seq = (self.seq + 1) & 0x3F
        common = bytearray(32)
        common[0:4] = bytes(self.sticks(state))
        face, shoulders, system = sony_buttons(state)
        common[4:7] = bytes([face, shoulders, (system & 0x03) | self.seq << 2])
        common[7:9] = bytes(self.triggers(state))
        common[29] = 0x08  # status: on battery
        if not self.bluetooth:
            report = bytearray(64)
            report[0] = 0x01
            report[1:33] = common
            return bytes(report)
        report = bytearray(78)
        report[0], report[1] = 0x11, 0xC0
        report[3:35] = common
        crc_sign(report, 0xA1, 74)
        return bytes(report)


# hid-nintendo's (and the Pro Controller's) button bytes 3-5 of a report.
NINTENDO_BUTTONS = {
    "west": (0, 0x01),  # Y
    "north": (0, 0x02),  # X
    "south": (0, 0x04),  # B
    "east": (0, 0x08),  # A
    "rb": (0, 0x40),
    "rt": (0, 0x80),
    "select": (1, 0x01),
    "start": (1, 0x02),
    "rs": (1, 0x04),
    "ls": (1, 0x08),
    "guide": (1, 0x10),
    "capture": (1, 0x20),
    HAT_DOWN: (2, 0x01),
    HAT_UP: (2, 0x02),
    HAT_RIGHT: (2, 0x04),
    HAT_LEFT: (2, 0x08),
    "lb": (2, 0x40),
    "lt": (2, 0x80),
}
STICK_CENTER, STICK_THROW = 0x800, 0x700  # the factory calibration tests/pads carries: 0x700 either side of 0x800


class Nintendo(Protocol):
    """A Pro Controller: the USB handshake, the subcommands hid-nintendo and SDL send and their 0x21 replies, the 0x30 report."""

    period = 0.015

    def __init__(self, pad, uniq):
        super().__init__(pad, uniq)
        self.timer = 0
        self.flash = {}
        for address, data in (pad.hid.get("spi") or {}).items():
            for i, b in enumerate(bytes.fromhex(data)):
                self.flash[int(address, 16) + i] = b
        self.last = State()

    def head(self, state, report_id):
        self.timer = (self.timer + 1) & 0xFF
        out = bytearray(64)
        out[0], out[1], out[2] = report_id, self.timer, 0x8E
        for slot, (index, bit) in NINTENDO_BUTTONS.items():
            if slot in state.pressed:
                out[3 + index] |= bit
        for at, (x_role, y_role) in ((6, ("lx", "ly")), (9, ("rx", "ry"))):
            x = round(STICK_CENTER + STICK_THROW * state.stick(x_role))
            y = round(STICK_CENTER - STICK_THROW * state.stick(y_role))
            out[at : at + 3] = bytes([x & 0xFF, (x >> 8) | ((y & 0x0F) << 4), y >> 4])
        out[12] = 0x0B
        return out

    def report(self, state):
        self.last = state
        return bytes(self.head(state, 0x30))

    def reply(self, ack, subcommand, data=b""):
        out = self.head(self.last, 0x21)
        out[13], out[14] = ack, subcommand
        out[15 : 15 + len(data)] = data
        return bytes(out)

    def output(self, data):
        if not data:
            return []
        if data[0] == 0x80 and len(data) > 1:
            command = data[1]
            if command == 0x01:
                return [bytes([0x81, 0x01, 0x00, 0x03, *mac_bytes(self.uniq)]).ljust(64, b"\0")]
            if command in (0x02, 0x03):
                return [bytes([0x81, command]).ljust(64, b"\0")]
            return []
        if data[0] != 0x01 or len(data) < 11:
            return []
        subcommand, args = data[10], data[11:]
        if subcommand == 0x02:
            address = bytes(int(b, 16) for b in self.uniq.split(":"))
            return [self.reply(0x82, 0x02, bytes([0x03, 0x48, 0x03, 0x02, *address, 0x01, 0x01]))]
        if subcommand == 0x10:
            start, size = struct.unpack_from("<IB", args)
            read = bytes(self.flash.get(start + i, 0xFF) for i in range(size))
            return [self.reply(0x90, 0x10, struct.pack("<IB", start, size) + read)]
        if subcommand == 0x04:
            return [self.reply(0x83, 0x04)]
        if subcommand == 0x21:
            return [self.reply(0xA0, 0x21, bytes([0x01, 0x00, 0xFF, 0x00, 0x03, 0x00, 0x05, 0x01]))]
        return [self.reply(0x80, subcommand)]


HID_TO_AXIS = {
    (hid.GENERIC_DESKTOP << 16) | 0x30: AXES["ABS_X"],
    (hid.GENERIC_DESKTOP << 16) | 0x31: AXES["ABS_Y"],
    (hid.GENERIC_DESKTOP << 16) | 0x32: AXES["ABS_Z"],
    (hid.GENERIC_DESKTOP << 16) | 0x33: AXES["ABS_RX"],
    (hid.GENERIC_DESKTOP << 16) | 0x34: AXES["ABS_RY"],
    (hid.GENERIC_DESKTOP << 16) | 0x35: AXES["ABS_RZ"],
    (hid.SIMULATION << 16) | 0xC4: AXES["ABS_GAS"],
    (hid.SIMULATION << 16) | 0xC5: AXES["ABS_BRAKE"],
}
HAT = (hid.GENERIC_DESKTOP << 16) | 0x39
JOYSTICK = (hid.GENERIC_DESKTOP << 16) | 0x04
# xpadneo: the Elite's paddles are one consumer field, a bit per paddle.
XPADNEO_PADDLES = ((hid.CONSUMER << 16) | 0x81, {"paddle_p1": 1, "paddle_p2": 2, "paddle_p3": 4, "paddle_p4": 8})


def hid_key(usage, application):
    """The evdev key hid-input gives a button or consumer usage."""
    page, n = usage >> 16, usage & 0xFFFF
    if page == hid.BUTTON and n:
        if n > 16:
            return 0x2C0 + n - 17
        return (0x120 if application == JOYSTICK else 0x130) + n - 1
    if usage == (hid.CONSUMER << 16) | 0xB2:
        return 167
    return None


class Generic(Protocol):
    """A pad hid-input reads off its descriptor (hid-generic, hid-microsoft, xpadneo): each control written into the field hid-input
    maps to the code tests/pads names for it. `hid.rest` is the report at rest (vendor bytes, the battery)."""

    def __init__(self, pad, uniq):
        super().__init__(pad, uniq)
        self.descriptor = hid.parse(bytes.fromhex(pad.hid["descriptor"]))
        self.report_id = pad.hid.get("report", next(iter(self.descriptor.inputs)))
        self.length = (self.descriptor.sizes[self.report_id] + 7) // 8 + (1 if self.report_id else 0)
        rest = bytes.fromhex(pad.hid.get("rest", "")) or bytes([self.report_id])
        self.rest = (rest + bytes(self.length))[: self.length]
        self.keys, self.axes = {}, {}
        for f in self.descriptor.inputs[self.report_id]:
            for i, usage in enumerate(f.usages if f.flags & hid.VARIABLE else []):
                code = hid_key(usage, self.descriptor.application)
                if code is not None:
                    self.keys.setdefault(code, (f, i))
                if usage in HID_TO_AXIS:
                    self.axes.setdefault(HID_TO_AXIS[usage], (f, i))
                if usage == HAT:
                    self.axes.setdefault("hat", (f, i))
                if usage == XPADNEO_PADDLES[0] and pad.driver == "xpadneo":
                    self.axes.setdefault("paddles", (f, i))

    def report(self, state):
        out = bytearray(self.rest)
        for slot, text in self.pad.buttons.items():
            kind, code, *_ = source(text)
            if kind == "key" and code in self.keys:
                f, i = self.keys[code]
                hid.put(out, f, i, int(slot in state.pressed))
        for role in (*STICKS, *TRIGGERS):
            axis = self.pad.axis(role)
            if axis is None or axis[0] not in self.axes:
                continue
            f, i = self.axes[axis[0]]
            trigger = role in TRIGGERS
            share = state.trigger(role) if trigger else ((-1 if axis[1] else 1) * state.stick(role) + 1) / 2
            hid.put(out, f, i, round(f.lo + share * (f.hi - f.lo)))
        if "hat" in self.axes:
            f, i = self.axes["hat"]
            index = hat_index(state.pressed)
            hid.put(out, f, i, f.lo + index if index < 8 else (0 if f.lo > 0 else f.hi + 1))
        if "paddles" in self.axes:
            f, i = self.axes["paddles"]
            hid.put(out, f, i, sum(bit for slot, bit in XPADNEO_PADDLES[1].items() if slot in state.pressed))
        return bytes(out)

    def feature(self, number):
        data = (self.pad.hid.get("features") or {}).get(f"{number:02x}")
        return bytes.fromhex(data) if data is not None else None


PROTOCOLS = {"dualsense": DualSense, "dualshock4": DualShock4, "nintendo": Nintendo, "generic": Generic}


# xpad's Xbox One (GIP) input packet: bytes 4 and 5.
GIP_BUTTONS = {
    "start": (4, 0x04),
    "select": (4, 0x08),
    "south": (4, 0x10),
    "east": (4, 0x20),
    "west": (4, 0x40),  # X
    "north": (4, 0x80),  # Y
    HAT_UP: (5, 0x01),
    HAT_DOWN: (5, 0x02),
    HAT_LEFT: (5, 0x04),
    HAT_RIGHT: (5, 0x08),
    "lb": (5, 0x10),
    "rb": (5, 0x20),
    "ls": (5, 0x40),
    "rs": (5, 0x80),
}
GIP_PADDLES = {"paddle_p1": 0x01, "paddle_p2": 0x02, "paddle_p3": 0x04, "paddle_p4": 0x08}


class XboxOne:
    """The GIP packets an Xbox One pad sends xpad: input (0x20), the guide on its own (0x07), an Elite 2's paddles (0x0c, firmware 5.11 on)."""

    LENGTH = 48

    def __init__(self, pad):
        self.pad = pad
        self.seq = 0
        self.guide = False

    def packets(self, state):
        self.seq = (self.seq + 1) & 0xFF
        out = []
        if ("guide" in state.pressed) != self.guide:
            self.guide = "guide" in state.pressed
            out.append(bytes([0x07, 0x20, self.seq, 0x02, int(self.guide), 0x5B]))
        data = bytearray(self.LENGTH)
        data[0], data[1], data[2], data[3] = 0x20, 0x00, self.seq, self.LENGTH - 4
        for slot, (index, bit) in GIP_BUTTONS.items():
            if slot in state.pressed:
                data[index] |= bit
        struct.pack_into("<HH", data, 6, round(state.trigger("lt") * 1023), round(state.trigger("rt") * 1023))
        lx, ly, rx, ry = (round(state.stick(r) * 32767) for r in STICKS)
        struct.pack_into("<hhhh", data, 10, lx, ~ly, rx, ~ry)
        if "share" in state.pressed:
            data[self.LENGTH - 26] |= 0x01
        out.append(bytes(data))
        if any(p in self.pad.buttons for p in GIP_PADDLES):
            firmware = bytearray(24)
            firmware[0], firmware[1], firmware[2], firmware[3] = 0x0C, 0x20, self.seq, 20
            firmware[18] = sum(bit for slot, bit in GIP_PADDLES.items() if slot in state.pressed)
            out.append(bytes(firmware))
        return out


# xpad's Xbox 360 report: bytes 2 and 3.
X360_BUTTONS = {
    HAT_UP: (2, 0x01),
    HAT_DOWN: (2, 0x02),
    HAT_LEFT: (2, 0x04),
    HAT_RIGHT: (2, 0x08),
    "start": (2, 0x10),
    "select": (2, 0x20),
    "ls": (2, 0x40),
    "rs": (2, 0x80),
    "lb": (3, 0x01),
    "rb": (3, 0x02),
    "guide": (3, 0x04),
    "south": (3, 0x10),
    "east": (3, 0x20),
    "west": (3, 0x40),
    "north": (3, 0x80),
}


class Xbox360:
    def __init__(self, pad):
        self.pad = pad

    def packets(self, state):
        data = bytearray(20)
        data[1] = 0x14
        for slot, (index, bit) in X360_BUTTONS.items():
            if slot in state.pressed:
                data[index] |= bit
        data[4], data[5] = round(state.trigger("lt") * 255), round(state.trigger("rt") * 255)
        lx, ly, rx, ry = (round(state.stick(r) * 32767) for r in STICKS)
        struct.pack_into("<hhhh", data, 6, lx, ~ly, rx, ~ry)
        return [bytes(data)]


class Silent:
    """A USB device no driver reads: it sends nothing."""

    def __init__(self, pad):
        self.pad = pad

    def packets(self, state):
        return []


GADGETS = {"xbox-one": XboxOne, "xbox-360": Xbox360, "none": Silent}
