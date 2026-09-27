import ctypes
import os
from dataclasses import dataclass, field
from typing import NamedTuple

# The flake writes the store path in; elsewhere the loader finds the distribution's.
LIBRARY = "@libSDL3@"

INIT_GAMEPAD = 0x2000
SENSOR_GYRO = 2
JOYSTICK_TYPE_GAMEPAD = 1

BIND_BUTTON, BIND_AXIS, BIND_HAT = 1, 2, 3

LABEL_A, LABEL_B, LABEL_X, LABEL_Y, LABEL_CROSS, LABEL_CIRCLE, LABEL_SQUARE, LABEL_TRIANGLE = 1, 2, 3, 4, 5, 6, 7, 8


class _GUID(ctypes.Structure):
    _fields_ = [("data", ctypes.c_uint8 * 16)]


class _Binding(ctypes.Structure):
    _fields_ = [("input_type", ctypes.c_int), ("input", ctypes.c_int * 3), ("output_type", ctypes.c_int), ("output", ctypes.c_int * 3)]


class Input(NamedTuple):
    """Where a gamepad button or axis reads off the raw joystick: a button, an axis over [lo, hi], or a hat direction (mask)."""

    kind: int
    index: int
    mask: int = 0
    lo: int = 0
    hi: int = 0


_P = ctypes.c_void_p
_SIGNATURES = {
    "SDL_SetHint": (ctypes.c_bool, [ctypes.c_char_p, ctypes.c_char_p]),
    "SDL_Init": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_Quit": (None, []),
    "SDL_free": (None, [_P]),
    "SDL_GetError": (ctypes.c_char_p, []),
    "SDL_GetJoysticks": (ctypes.POINTER(ctypes.c_uint32), [ctypes.POINTER(ctypes.c_int)]),
    "SDL_GetJoystickGUIDForID": (_GUID, [ctypes.c_uint32]),
    "SDL_GetJoystickNameForID": (ctypes.c_char_p, [ctypes.c_uint32]),
    "SDL_GetJoystickPlayerIndexForID": (ctypes.c_int, [ctypes.c_uint32]),
    "SDL_GetJoystickTypeForID": (ctypes.c_int, [ctypes.c_uint32]),
    "SDL_IsGamepad": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_OpenGamepad": (_P, [ctypes.c_uint32]),
    "SDL_CloseGamepad": (None, [_P]),
    "SDL_GetGamepadName": (ctypes.c_char_p, [_P]),
    "SDL_GetGamepadJoystick": (_P, [_P]),
    "SDL_GetNumJoystickAxes": (ctypes.c_int, [_P]),
    "SDL_GetGamepadMapping": (_P, [_P]),
    "SDL_GetGamepadBindings": (ctypes.POINTER(ctypes.POINTER(_Binding)), [_P, ctypes.POINTER(ctypes.c_int)]),
    "SDL_GetGamepadButtonLabel": (ctypes.c_int, [_P, ctypes.c_int]),
    "SDL_GamepadHasSensor": (ctypes.c_bool, [_P, ctypes.c_int]),
}


@dataclass
class Pad:
    name: str
    guid: bytes
    # (output_type, output index) → the first binding for it, as SDL2's GetBindFor* returns it.
    bindings: dict = field(default_factory=dict)
    gyro: bool = False
    # Position in SDL_GetJoysticks, non-gamepads counted: the device index SDL2 frontends open.
    index: int = 0
    joystick_name: str = ""
    player_index: int = -1
    gamepad_type: bool = True
    axes: int = 0
    # The labels printed on SOUTH, EAST, WEST, NORTH.
    labels: tuple = (LABEL_A, LABEL_B, LABEL_X, LABEL_Y)
    mapping: str = ""


def load():
    path = LIBRARY if os.path.isabs(LIBRARY) else "libSDL3.so.0"
    lib = ctypes.CDLL(path)
    for name, (restype, argtypes) in _SIGNATURES.items():
        fn = getattr(lib, name)
        fn.restype = restype
        fn.argtypes = argtypes
    return lib


def _bindings(sdl, handle):
    count = ctypes.c_int(0)
    arr = sdl.SDL_GetGamepadBindings(handle, ctypes.byref(count))
    if not arr:
        return {}
    out = {}
    try:
        for i in range(count.value):
            b = arr[i].contents
            if b.output_type not in (BIND_BUTTON, BIND_AXIS) or b.input_type not in (BIND_BUTTON, BIND_AXIS, BIND_HAT):
                continue
            key = (b.output_type, b.output[0])
            if key in out:
                continue
            if b.input_type == BIND_HAT:
                out[key] = Input(BIND_HAT, b.input[0], mask=b.input[1])
            elif b.input_type == BIND_AXIS:
                out[key] = Input(BIND_AXIS, b.input[0], lo=b.input[1], hi=b.input[2])
            else:
                out[key] = Input(BIND_BUTTON, b.input[0])
    finally:
        sdl.SDL_free(arr)
    return out


def _string(sdl, ptr):
    if not ptr:
        return ""
    try:
        return ctypes.string_at(ptr).decode(errors="replace")
    finally:
        sdl.SDL_free(ptr)


def _pad(sdl, jid, index, handle):
    joystick = sdl.SDL_GetGamepadJoystick(handle)
    return Pad(
        name=(sdl.SDL_GetGamepadName(handle) or b"").decode(errors="replace"),
        guid=bytes(sdl.SDL_GetJoystickGUIDForID(jid).data),
        bindings=_bindings(sdl, handle),
        gyro=bool(sdl.SDL_GamepadHasSensor(handle, SENSOR_GYRO)),
        index=index,
        joystick_name=(sdl.SDL_GetJoystickNameForID(jid) or b"").decode(errors="replace"),
        player_index=sdl.SDL_GetJoystickPlayerIndexForID(jid),
        gamepad_type=sdl.SDL_GetJoystickTypeForID(jid) == JOYSTICK_TYPE_GAMEPAD,
        axes=sdl.SDL_GetNumJoystickAxes(joystick) if joystick else 0,
        labels=tuple(sdl.SDL_GetGamepadButtonLabel(handle, b) for b in range(4)),
        mapping=_string(sdl, sdl.SDL_GetGamepadMapping(handle)),
    )


def pads(hints):
    sdl = load()
    for k, v in hints.items():
        sdl.SDL_SetHint(k.encode(), v.encode())
    if not sdl.SDL_Init(INIT_GAMEPAD):
        raise OSError(f"SDL_Init: {(sdl.SDL_GetError() or b'').decode(errors='replace')}")
    try:
        count = ctypes.c_int(0)
        ids = sdl.SDL_GetJoysticks(ctypes.byref(count))
        found = []
        try:
            for i in range(count.value):
                jid = ids[i]
                if not sdl.SDL_IsGamepad(jid):
                    continue
                handle = sdl.SDL_OpenGamepad(jid)
                if not handle:
                    continue
                try:
                    found.append(_pad(sdl, jid, i, handle))
                finally:
                    sdl.SDL_CloseGamepad(handle)
        finally:
            sdl.SDL_free(ids)
        return found
    finally:
        sdl.SDL_Quit()
