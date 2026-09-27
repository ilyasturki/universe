import ctypes
import os
import struct

from _pads import PadState

# The flake writes the store path in; elsewhere the loader finds the distribution's.
LIBRARY = "@libSDL3@"

INIT_GAMEPAD = 0x2000

EVENT_QUIT = 0x100
EVENT_GAMEPAD_AXIS_MOTION = 0x650
EVENT_GAMEPAD_BUTTON_DOWN = 0x651
EVENT_GAMEPAD_BUTTON_UP = 0x652
EVENT_GAMEPAD_ADDED = 0x653
EVENT_GAMEPAD_REMOVED = 0x654
EVENT_GAMEPAD_TOUCHPAD_DOWN = 0x656
EVENT_GAMEPAD_TOUCHPAD_MOTION = 0x657
EVENT_GAMEPAD_TOUCHPAD_UP = 0x658
EVENT_GAMEPAD_SENSOR_UPDATE = 0x659
EVENT_GAMEPAD_UPDATE_COMPLETE = 0x65A

BUTTON_SOUTH, BUTTON_EAST, BUTTON_WEST, BUTTON_NORTH = 0, 1, 2, 3
BUTTON_BACK, BUTTON_GUIDE, BUTTON_START, BUTTON_LEFT_STICK, BUTTON_RIGHT_STICK = 4, 5, 6, 7, 8
BUTTON_LEFT_SHOULDER, BUTTON_RIGHT_SHOULDER = 9, 10
BUTTON_DPAD_UP, BUTTON_DPAD_DOWN, BUTTON_DPAD_LEFT, BUTTON_DPAD_RIGHT = 11, 12, 13, 14
BUTTON_MISC1, BUTTON_TOUCHPAD = 15, 20
BUTTON_COUNT = 21
AXIS_COUNT = 6

SENSOR_ACCEL, SENSOR_GYRO = 1, 2

POWERSTATE_ON_BATTERY, POWERSTATE_NO_BATTERY, POWERSTATE_CHARGING, POWERSTATE_CHARGED = 2, 3, 4, 5

EVENT_SIZE = 128

# Past the 2 s at which the emulator's SDL resends a held rumble, so a held one does not drop in between.
RUMBLE_HOLD_MS = 3000

_P = ctypes.c_void_p
_SIGNATURES = {
    "SDL_SetHint": (ctypes.c_bool, [ctypes.c_char_p, ctypes.c_char_p]),
    "SDL_Init": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_Quit": (None, []),
    "SDL_GetError": (ctypes.c_char_p, []),
    "SDL_SetEventEnabled": (None, [ctypes.c_uint32, ctypes.c_bool]),
    "SDL_WaitEventTimeout": (ctypes.c_bool, [_P, ctypes.c_int32]),
    "SDL_PollEvent": (ctypes.c_bool, [_P]),
    "SDL_OpenGamepad": (_P, [ctypes.c_uint32]),
    "SDL_CloseGamepad": (None, [_P]),
    "SDL_GetGamepadName": (ctypes.c_char_p, [_P]),
    "SDL_GetGamepadVendor": (ctypes.c_uint16, [_P]),
    "SDL_GetGamepadProduct": (ctypes.c_uint16, [_P]),
    "SDL_GetGamepadButton": (ctypes.c_bool, [_P, ctypes.c_int]),
    "SDL_GetGamepadAxis": (ctypes.c_int16, [_P, ctypes.c_int]),
    "SDL_GamepadHasSensor": (ctypes.c_bool, [_P, ctypes.c_int]),
    "SDL_SetGamepadSensorEnabled": (ctypes.c_bool, [_P, ctypes.c_int, ctypes.c_bool]),
    "SDL_GetGamepadSensorData": (ctypes.c_bool, [_P, ctypes.c_int, ctypes.POINTER(ctypes.c_float), ctypes.c_int]),
    "SDL_GetNumGamepadTouchpads": (ctypes.c_int, [_P]),
    "SDL_GetGamepadTouchpadFinger": (
        ctypes.c_bool,
        [
            _P,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.POINTER(ctypes.c_bool),
            ctypes.POINTER(ctypes.c_float),
            ctypes.POINTER(ctypes.c_float),
            ctypes.POINTER(ctypes.c_float),
        ],
    ),
    "SDL_GetGamepadPowerInfo": (ctypes.c_int, [_P, ctypes.POINTER(ctypes.c_int)]),
    "SDL_RumbleGamepad": (ctypes.c_bool, [_P, ctypes.c_uint16, ctypes.c_uint16, ctypes.c_uint32]),
}


def load():
    path = LIBRARY if os.path.isabs(LIBRARY) else "libSDL3.so.0"
    lib = ctypes.CDLL(path)
    for name, (restype, argtypes) in _SIGNATURES.items():
        fn = getattr(lib, name)
        fn.restype = restype
        fn.argtypes = argtypes
    return lib


def event_kind(buf):
    return struct.unpack_from("<I", buf, 0)[0]


def event_which(buf):
    return struct.unpack_from("<I", buf, 16)[0]


class Gamepad:
    """A physical pad as SDL reads it: its state, sampled when a report is due."""

    def __init__(self, sdl, handle, instance_id):
        self.sdl, self.handle, self.id = sdl, handle, instance_id
        self.name = (sdl.SDL_GetGamepadName(handle) or b"").decode(errors="replace")
        self.vendor = sdl.SDL_GetGamepadVendor(handle)
        self.product = sdl.SDL_GetGamepadProduct(handle)
        self.gyro = sdl.SDL_GamepadHasSensor(handle, SENSOR_GYRO) and sdl.SDL_SetGamepadSensorEnabled(handle, SENSOR_GYRO, True)
        self.accel = sdl.SDL_GamepadHasSensor(handle, SENSOR_ACCEL) and sdl.SDL_SetGamepadSensorEnabled(handle, SENSOR_ACCEL, True)
        self.touchpad = sdl.SDL_GetNumGamepadTouchpads(handle) > 0

    def _sensor(self, kind):
        out = (ctypes.c_float * 3)()
        if not self.sdl.SDL_GetGamepadSensorData(self.handle, kind, out, 3):
            return None
        return (out[0], out[1], out[2])

    def _finger(self, finger):
        down, x, y, pressure = ctypes.c_bool(), ctypes.c_float(), ctypes.c_float(), ctypes.c_float()
        if not self.sdl.SDL_GetGamepadTouchpadFinger(self.handle, 0, finger, ctypes.byref(down), ctypes.byref(x), ctypes.byref(y), ctypes.byref(pressure)):
            return None
        return (x.value, y.value) if down.value else None

    def state(self):
        percent = ctypes.c_int(-1)
        power = self.sdl.SDL_GetGamepadPowerInfo(self.handle, ctypes.byref(percent))
        return PadState(
            buttons=frozenset(b for b in range(BUTTON_COUNT) if self.sdl.SDL_GetGamepadButton(self.handle, b)),
            axes=tuple(self.sdl.SDL_GetGamepadAxis(self.handle, a) for a in range(AXIS_COUNT)),
            gyro=self._sensor(SENSOR_GYRO) if self.gyro else None,
            accel=self._sensor(SENSOR_ACCEL) if self.accel else None,
            touches=(self._finger(0), self._finger(1)) if self.touchpad else (None, None),
            power=(power, percent.value),
        )

    def rumble(self, low, high):
        self.sdl.SDL_RumbleGamepad(self.handle, low, high, RUMBLE_HOLD_MS)

    def close(self):
        self.sdl.SDL_CloseGamepad(self.handle)
