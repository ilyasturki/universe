import json
import math
import os
import struct
import sys
from dataclasses import dataclass

# DragonRise: SDL sends an unlisted product of it the PlayStation capability query, and no listing of 0x555x makes it another type.
VENDOR = 0x0079
FIRST_PRODUCT = 0x5550
MAX_PLAYERS = 8
UHID = "/dev/uhid"

UHID_DESTROY, UHID_START, UHID_OUTPUT, UHID_GET_REPORT, UHID_GET_REPORT_REPLY = 1, 2, 6, 9, 10
UHID_CREATE2, UHID_INPUT2, UHID_SET_REPORT, UHID_SET_REPORT_REPLY = 11, 12, 13, 14
UHID_EVENT_SIZE = 4380
BUS_USB = 0x03
EIO = 5

# Generic Desktop / Game Pad on the outside, for SDL's filter; vendor usages inside, so hid-input makes no evdev node of it.
REPORT_DESCRIPTOR = bytes(
    [
        0x05, 0x01, 0x09, 0x05, 0xA1, 0x01,
        0x06, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08,
        0x85, 0x01, 0x09, 0x20, 0x95, 0x3F, 0x81, 0x02,
        0x85, 0x02, 0x09, 0x21, 0x95, 0x2F, 0x91, 0x02,
        0x85, 0x03, 0x09, 0x22, 0x95, 0x2F, 0xB1, 0x02,
        0x85, 0x05, 0x09, 0x23, 0x95, 0x28, 0xB1, 0x02,
        0xC0,
    ]
)  # fmt: skip

REPORT_STATE, REPORT_EFFECTS, REPORT_CAPABILITIES = 0x01, 0x02, 0x03
# SDL_hidapi_ps5.c's third-party query: 48 bytes, 0x28 at [2], [4] sensors 0x02 | vibration 0x08 | touchpad 0x40, [5] 0 a gamepad.
CAPABILITIES = bytes([REPORT_CAPABILITIES, 0, 0x28, 0, 0x02 | 0x08 | 0x40]).ljust(48, b"\0")

# No calibration report: SDL reads gyro as 1/16 °/s and accel as 1/8192 g.
GYRO_PER_RAD_S = 16 * 180 / math.pi
ACCEL_PER_M_S2 = 8192 / 9.80665
TOUCH_W, TOUCH_H = 1920, 1070
REST_ACCEL = (0.0, 9.80665, 0.0)

HAT = {(1, 0, 0, 0): 0, (1, 1, 0, 0): 1, (0, 1, 0, 0): 2, (0, 1, 1, 0): 3, (0, 0, 1, 0): 4, (0, 0, 1, 1): 5, (0, 0, 0, 1): 6, (1, 0, 0, 1): 7}
HAT_CENTERED = 8
BATTERY_UNKNOWN = 0x0C

SOUTH, EAST, WEST, NORTH, BACK, GUIDE, START, LEFT_STICK, RIGHT_STICK, LEFT_SHOULDER, RIGHT_SHOULDER = range(11)
DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT, MISC1 = 11, 12, 13, 14, 15
TOUCHPAD = 20
FACE_BITS = ((WEST, 0x10), (SOUTH, 0x20), (EAST, 0x40), (NORTH, 0x80))
SHOULDER_BITS = ((LEFT_SHOULDER, 0x01), (RIGHT_SHOULDER, 0x02), (BACK, 0x10), (START, 0x20), (LEFT_STICK, 0x40), (RIGHT_STICK, 0x80))
SYSTEM_BITS = ((GUIDE, 0x01), (TOUCHPAD, 0x02), (MISC1, 0x04))

POWER_ON_BATTERY, POWER_CHARGING, POWER_CHARGED = 2, 4, 5


@dataclass(frozen=True)
class PadState:
    buttons: frozenset = frozenset()
    axes: tuple = (0, 0, 0, 0, 0, 0)
    gyro: tuple | None = None
    accel: tuple | None = None
    touches: tuple = (None, None)
    power: tuple = (0, -1)


NEUTRAL = PadState()


def log(msg):
    print(f"[pads] {msg}", file=sys.stderr, flush=True)


def product(player):
    return FIRST_PRODUCT + player


def ignore_list():
    return ",".join(f"0x{VENDOR:04x}/0x{product(p):04x}" for p in range(MAX_PLAYERS))


def game_env():
    return {"SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT": ignore_list(), "SDL_JOYSTICK_HIDAPI_PS5": "1"}


def state_path(session_id):
    runtime = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
    return os.path.join(runtime, "universe", f"pads-{session_id}.json")


def write_state(path, doc):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = f"{path}.tmp"
    with open(tmp, "w") as f:
        json.dump(doc, f)
    os.replace(tmp, path)


def unit_name(session_id):
    return f"universe-pads-{session_id}.service"


def _clamp(v, lo, hi):
    return max(lo, min(hi, v))


def _stick(v):
    return _clamp((v + 32768) >> 8, 0, 255)


def _trigger(v):
    return _clamp(round(v * 255 / 32767), 0, 255)


def _bits(buttons, table):
    return sum(bit for button, bit in table if button in buttons)


def _hat(buttons):
    key = (int(DPAD_UP in buttons), int(DPAD_RIGHT in buttons), int(DPAD_DOWN in buttons), int(DPAD_LEFT in buttons))
    return HAT.get(key, HAT_CENTERED)


def _vec(values, scale):
    return struct.pack("<hhh", *(_clamp(round(v * scale), -32768, 32767) for v in values))


def _touch(finger, point):
    if point is None:
        return bytes([0x80, 0, 0, 0])
    x = _clamp(round(point[0] * TOUCH_W), 0, TOUCH_W - 1)
    y = _clamp(round(point[1] * TOUCH_H), 0, TOUCH_H - 1)
    return bytes([finger & 0x7F, x & 0xFF, (x >> 8) | ((y & 0x0F) << 4), y >> 4])


def _battery(power):
    state, percent = power
    level = _clamp(percent // 10, 0, 10)
    if state == POWER_ON_BATTERY and percent >= 0:
        return level
    if state == POWER_CHARGING and percent >= 0:
        return 0x10 | level
    if state == POWER_CHARGED:
        return 0x20 | 10
    return BATTERY_UNKNOWN


def pack_report(state, seq, tick_us, guide=True):
    """The USB state report SDL's PS5 driver parses for a third-party pad (PS5StatePacketAlt_t), report id first."""
    buttons = state.buttons if guide else state.buttons - {GUIDE}
    lx, ly, rx, ry, lt, rt = state.axes
    return b"".join(
        [
            bytes([REPORT_STATE, _stick(lx), _stick(ly), _stick(rx), _stick(ry), _trigger(lt), _trigger(rt), seq & 0xFF]),
            bytes([_hat(buttons) | _bits(buttons, FACE_BITS), _bits(buttons, SHOULDER_BITS), _bits(buttons, SYSTEM_BITS), 0]),
            struct.pack("<I", seq & 0xFFFFFFFF),
            _vec(state.gyro or (0.0, 0.0, 0.0), GYRO_PER_RAD_S),
            _vec(state.accel or REST_ACCEL, ACCEL_PER_M_S2),
            struct.pack("<H", tick_us & 0xFFFF),
            bytes([_battery(state.power), 0]),
            _touch(0, state.touches[0]),
            _touch(1, state.touches[1]),
        ]
    ).ljust(64, b"\0")


def parse_rumble(data):
    """(low, high) motor strengths from SDL's effects report, None for any other output."""
    if len(data) < 5 or data[0] != REPORT_EFFECTS:
        return None
    if not data[1] & 0x01:
        return (0, 0)
    # SDL halves a third-party pad's rumble to match an Xbox pad's strength.
    return (min(0xFFFF, (data[4] << 1) * 257), min(0xFFFF, (data[3] << 1) * 257))


class VirtualPad:
    """One player's pad over /dev/uhid: fixed name, product and uniq, so the emulator's SDL gives it the same GUID every session."""

    def __init__(self, player):
        self.player = player
        self.name = f"Universe Pad {player + 1}"
        self.started = False
        self.seq = 0
        self.fd = os.open(UHID, os.O_RDWR | os.O_CLOEXEC)
        create = struct.pack(
            "<I128s64s64sHHIIII",
            UHID_CREATE2,
            self.name.encode(),
            b"",
            f"universe-pad-{player + 1}".encode(),
            len(REPORT_DESCRIPTOR),
            BUS_USB,
            VENDOR,
            product(player),
            1,
            0,
        )
        os.write(self.fd, create + REPORT_DESCRIPTOR)

    def send(self, state, tick_us, guide):
        self.seq += 1
        report = pack_report(state, self.seq, tick_us, guide)
        os.write(self.fd, struct.pack("<IH", UHID_INPUT2, len(report)) + report)

    def handle(self):
        """Answers the kernel's request; a rumble the emulator sent comes back as (low, high)."""
        buf = os.read(self.fd, UHID_EVENT_SIZE)
        kind = struct.unpack_from("<I", buf)[0]
        if kind == UHID_START:
            self.started = True
        elif kind == UHID_GET_REPORT:
            request, number = struct.unpack_from("<IB", buf, 4)
            if number == REPORT_CAPABILITIES:
                os.write(self.fd, struct.pack("<IIHH", UHID_GET_REPORT_REPLY, request, 0, len(CAPABILITIES)) + CAPABILITIES)
            else:
                os.write(self.fd, struct.pack("<IIHH", UHID_GET_REPORT_REPLY, request, EIO, 0))
        elif kind == UHID_SET_REPORT:
            request = struct.unpack_from("<I", buf, 4)[0]
            os.write(self.fd, struct.pack("<IIH", UHID_SET_REPORT_REPLY, request, 0))
        elif kind == UHID_OUTPUT:
            size = struct.unpack_from("<H", buf, 4 + 4096)[0]
            return parse_rumble(buf[4 : 4 + size])
        return None

    def close(self):
        try:
            os.write(self.fd, struct.pack("<I", UHID_DESTROY))
        finally:
            os.close(self.fd)
