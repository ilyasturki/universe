import ctypes
import os
from dataclasses import dataclass, field

# The flake writes the store path in; elsewhere the loader finds the distribution's.
LIBRARY = "@libSDL3@"

INIT_GAMEPAD = 0x2000
SENSOR_GYRO = 2

BIND_BUTTON, BIND_AXIS, BIND_HAT = 1, 2, 3


class _GUID(ctypes.Structure):
    _fields_ = [("data", ctypes.c_uint8 * 16)]


class _Binding(ctypes.Structure):
    _fields_ = [("input_type", ctypes.c_int), ("input", ctypes.c_int * 3), ("output_type", ctypes.c_int), ("output", ctypes.c_int * 3)]


_P = ctypes.c_void_p
_SIGNATURES = {
    "SDL_SetHint": (ctypes.c_bool, [ctypes.c_char_p, ctypes.c_char_p]),
    "SDL_Init": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_Quit": (None, []),
    "SDL_free": (None, [_P]),
    "SDL_GetError": (ctypes.c_char_p, []),
    "SDL_GetJoysticks": (ctypes.POINTER(ctypes.c_uint32), [ctypes.POINTER(ctypes.c_int)]),
    "SDL_GetJoystickGUIDForID": (_GUID, [ctypes.c_uint32]),
    "SDL_IsGamepad": (ctypes.c_bool, [ctypes.c_uint32]),
    "SDL_OpenGamepad": (_P, [ctypes.c_uint32]),
    "SDL_CloseGamepad": (None, [_P]),
    "SDL_GetGamepadName": (ctypes.c_char_p, [_P]),
    "SDL_GetGamepadBindings": (ctypes.POINTER(ctypes.POINTER(_Binding)), [_P, ctypes.POINTER(ctypes.c_int)]),
    "SDL_GamepadHasSensor": (ctypes.c_bool, [_P, ctypes.c_int]),
}


@dataclass
class Pad:
    name: str
    guid: bytes
    # (output_type, output index) → (input_type, input index, hat mask); the first binding per output, as SDL2's GetBindFor* returns it.
    bindings: dict = field(default_factory=dict)
    gyro: bool = False


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
            if key not in out:
                out[key] = (b.input_type, b.input[0], b.input[1] if b.input_type == BIND_HAT else 0)
    finally:
        sdl.SDL_free(arr)
    return out


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
                    found.append(
                        Pad(
                            name=(sdl.SDL_GetGamepadName(handle) or b"").decode(errors="replace"),
                            guid=bytes(sdl.SDL_GetJoystickGUIDForID(jid).data),
                            bindings=_bindings(sdl, handle),
                            gyro=bool(sdl.SDL_GamepadHasSensor(handle, SENSOR_GYRO)),
                        )
                    )
                finally:
                    sdl.SDL_CloseGamepad(handle)
        finally:
            sdl.SDL_free(ids)
        return found
    finally:
        sdl.SDL_Quit()
