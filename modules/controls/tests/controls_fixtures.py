import importlib.machinery
import importlib.util
import json
from pathlib import Path

from _gamepads import BIND_AXIS, BIND_BUTTON, Input, Pad, from_view

BIN_DIR = Path(__file__).resolve().parents[1] / "bin"
CORPUS = Path(__file__).resolve().parents[3] / "tests" / "pads"

B, A = BIND_BUTTON, BIND_AXIS


def recorded(name, **session):
    return from_view(json.loads((CORPUS / f"{name}.json").read_text())["sdl"], **session)


# SDL3's HIDAPI PS5 driver on a DualSense Edge over Bluetooth.
EDGE = recorded("dualsense-edge-bt", index=0, player_index=0, device="/sys/devices/virtual/misc/uhid/0005:054C:0DF2.0003")
# The Linux joystick driver's numbering of an Xbox pad (xpadneo): buttons by evdev code, triggers on axes 2 and 5.
XBOX = recorded("xbox-elite-bt-xpadneo", index=1, player_index=1)
# SDL3's HIDAPI Switch driver on a Pro Controller: B on the bottom, A on the right, digital triggers.
SWITCH_PRO = recorded("switch-pro-usb", index=2, player_index=2)
# A Pro Controller whose ZL and ZR read as buttons, as on a driver without analog triggers.
SWITCH_PRO_DIGITAL = Pad(**{**vars(SWITCH_PRO), "bindings": {**SWITCH_PRO.bindings, (A, 4): Input(B, 17), (A, 5): Input(B, 18)}})
# SDL3's HIDAPI 8BitDo driver on a Pro 3 in D-input mode over Bluetooth: raw buttons by printed label (b0 the A on the right), B at the bottom.
PRO3_DINPUT = recorded("8bitdo-pro-3-dinput-bt", index=3, player_index=3, device="/sys/devices/virtual/misc/uhid/0005:2DC8:6009.000E")
# The Pro 3 in X-input mode is an Xbox 360 pad to xpad: the bottom button, printed B, is BTN_A and SDL labels it A.
PRO3_XINPUT = recorded("8bitdo-pro-3-xinput-usb", index=4, player_index=4)

# Every recorded pad with SDL3's view of it, as one session would list them all.
RECORDED = {
    path.stem: recorded(path.stem, index=n, player_index=n)
    for n, path in enumerate(p for p in sorted(CORPUS.glob("*.json")) if "sdl" in json.loads(p.read_text()))
}


def twin(pad):
    return Pad(**{**vars(pad), "index": pad.index + 10, "player_index": pad.player_index + 10, "device": pad.device + "-twin"})


def script(name):
    loader = importlib.machinery.SourceFileLoader(f"controls_{name}", str(BIN_DIR / name))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod
