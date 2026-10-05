import importlib.machinery
import importlib.util
from pathlib import Path

from _gamepads import BIND_AXIS, BIND_BUTTON, BIND_HAT, Input, Pad

BIN_DIR = Path(__file__).resolve().parents[1] / "bin"

B, A, H = BIND_BUTTON, BIND_AXIS, BIND_HAT
FULL = {"lo": -32768, "hi": 32767}


def _dpad(hat=0):
    return {(B, 11): Input(H, hat, 1), (B, 12): Input(H, hat, 4), (B, 13): Input(H, hat, 8), (B, 14): Input(H, hat, 2)}


def _axes(order):
    return {(A, out): Input(A, raw, **FULL) for out, raw in enumerate(order)}


# SDL3's HIDAPI PS5 driver, read off a DualSense Edge over Bluetooth.
EDGE = Pad(
    name="DualSense Edge Wireless Controller",
    guid=bytes.fromhex("0500e0274c050000f20d000000006800"),
    bindings={
        **{(B, i): Input(B, i) for i in range(11)},
        **_dpad(),
        (B, 15): Input(B, 12),
        (B, 16): Input(B, 16),
        (B, 17): Input(B, 15),
        (B, 18): Input(B, 14),
        (B, 19): Input(B, 13),
        (B, 20): Input(B, 11),
        **_axes(range(6)),
    },
    gyro=True,
    index=0,
    joystick_name="DualSense Edge Wireless Controller",
    player_index=0,
    axes=6,
    labels=(5, 6, 7, 8),
    device="/sys/devices/virtual/misc/uhid/0005:054C:0DF2.0003",
    mapping="0500e0274c050000f20d000000006800,*,a:b0,b:b1,back:b4,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b5,"
    "leftshoulder:b9,leftstick:b7,lefttrigger:a4,leftx:a0,lefty:a1,rightshoulder:b10,rightstick:b8,righttrigger:a5,rightx:a2,righty:a3,"
    "start:b6,x:b2,y:b3,touchpad:b11,misc1:b12,paddle1:b16,paddle2:b15,paddle3:b14,paddle4:b13,crc:27e0,platform:Linux,",
)

# The Linux joystick driver's numbering of an Xbox pad (xpadneo): buttons by evdev code, triggers on axes 2 and 5.
XBOX = Pad(
    name="Xbox Wireless Controller",
    guid=bytes.fromhex("0500509a5e0400008e02000030110000"),
    bindings={
        (B, 0): Input(B, 0),
        (B, 1): Input(B, 1),
        (B, 2): Input(B, 2),
        (B, 3): Input(B, 3),
        (B, 4): Input(B, 6),
        (B, 5): Input(B, 8),
        (B, 6): Input(B, 7),
        (B, 7): Input(B, 9),
        (B, 8): Input(B, 10),
        (B, 9): Input(B, 4),
        (B, 10): Input(B, 5),
        **_dpad(),
        **_axes((0, 1, 3, 4, 2, 5)),
    },
    index=1,
    joystick_name="Xbox Wireless Controller",
    player_index=1,
    axes=6,
    labels=(1, 2, 3, 4),
    mapping="0500509a5e0400008e02000030110000,Xbox Wireless Controller,a:b0,b:b1,x:b2,y:b3,back:b6,guide:b8,start:b7,leftstick:b9,"
    "rightstick:b10,leftshoulder:b4,rightshoulder:b5,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,leftx:a0,lefty:a1,rightx:a3,"
    "righty:a4,lefttrigger:a2,righttrigger:a5,paddle1:b11,paddle2:b13,paddle3:b12,paddle4:b14,platform:Linux,",
)

# SDL3's HIDAPI Switch driver on a Pro Controller: B on the bottom, A on the right, digital triggers.
SWITCH_PRO = Pad(
    name="Nintendo Switch Pro Controller",
    guid=bytes.fromhex("03004d8f7e0500000920000000006800"),
    bindings={
        **{(B, i): Input(B, i) for i in range(11)},
        **_dpad(),
        (B, 15): Input(B, 11),
        **_axes(range(4)),
        (A, 4): Input(A, 4, **FULL),
        (A, 5): Input(A, 5, **FULL),
    },
    gyro=True,
    index=2,
    joystick_name="Nintendo Switch Pro Controller",
    player_index=2,
    axes=6,
    labels=(2, 1, 4, 3),
    mapping="03004d8f7e0500000920000000006800,*,a:b0,b:b1,x:b2,y:b3,back:b4,guide:b5,start:b6,leftstick:b7,rightstick:b8,"
    "leftshoulder:b9,rightshoulder:b10,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,leftx:a0,lefty:a1,rightx:a2,righty:a3,"
    "lefttrigger:a4,righttrigger:a5,misc1:b11,crc:8f4d,platform:Linux,",
)


# A Pro Controller whose ZL and ZR read as buttons, as on a driver without analog triggers.
SWITCH_PRO_DIGITAL = Pad(**{**vars(SWITCH_PRO), "bindings": {**SWITCH_PRO.bindings, (A, 4): Input(B, 17), (A, 5): Input(B, 18)}})

# SDL3's HIDAPI 8BitDo driver on a Pro 3 in D-input mode over Bluetooth: raw buttons by printed label (b0 the A on the right), B at the bottom.
PRO3_DINPUT = Pad(
    name="8BitDo Pro 3",
    guid=bytes.fromhex("0500546cc82d00000960000000006800"),
    bindings={
        (B, 0): Input(B, 1),
        (B, 1): Input(B, 0),
        (B, 2): Input(B, 3),
        (B, 3): Input(B, 2),
        **{(B, i): Input(B, i) for i in range(4, 11)},
        **_dpad(),
        (B, 16): Input(B, 12),
        (B, 17): Input(B, 11),
        (B, 18): Input(B, 14),
        (B, 19): Input(B, 13),
        **_axes(range(6)),
    },
    gyro=True,
    index=3,
    joystick_name="8BitDo Pro 3",
    player_index=3,
    axes=6,
    labels=(2, 1, 4, 3),
    device="/sys/devices/virtual/misc/uhid/0005:2DC8:6009.000E",
    mapping="0500546cc82d00000960000000006800,*,a:b1,b:b0,back:b4,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b5,"
    "leftshoulder:b9,leftstick:b7,lefttrigger:a4,leftx:a0,lefty:a1,rightshoulder:b10,rightstick:b8,righttrigger:a5,rightx:a2,righty:a3,"
    "start:b6,x:b3,y:b2,hint:!SDL_GAMECONTROLLER_USE_BUTTON_LABELS:=1,paddle1:b12,paddle2:b11,paddle3:b14,paddle4:b13,crc:6c54,platform:Linux,",
)

# The Pro 3 in X-input mode is an Xbox 360 pad to xpad: the bottom button, printed B, is BTN_A and SDL labels it A.
PRO3_XINPUT = Pad(
    name="Xbox 360 Controller",
    guid=bytes.fromhex("030000005e0400008e02000014010000"),
    bindings={
        **{(B, i): Input(B, i) for i in range(4)},
        (B, 4): Input(B, 6),
        (B, 5): Input(B, 8),
        (B, 6): Input(B, 7),
        (B, 7): Input(B, 9),
        (B, 8): Input(B, 10),
        (B, 9): Input(B, 4),
        (B, 10): Input(B, 5),
        **_dpad(),
        **_axes((0, 1, 3, 4, 2, 5)),
    },
    index=4,
    joystick_name="Xbox 360 Controller",
    player_index=4,
    axes=6,
    labels=(1, 2, 3, 4),
    mapping="030000005e0400008e02000014010000,Xbox 360 Controller,a:b0,b:b1,back:b6,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b8,"
    "leftshoulder:b4,leftstick:b9,lefttrigger:a2,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b10,righttrigger:a5,rightx:a3,righty:a4,"
    "start:b7,x:b2,y:b3,platform:Linux,",
)


def twin(pad):
    return Pad(**{**vars(pad), "index": pad.index + 10, "player_index": pad.player_index + 10, "device": pad.device + "-twin"})


def script(name):
    loader = importlib.machinery.SourceFileLoader(f"controls_{name}", str(BIN_DIR / name))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod
