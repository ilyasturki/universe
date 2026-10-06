import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bin"))
import _gamepads
import controls_fixtures
from _gamepads import Pad


# CONTROLS_PADS: SDL's list, as controls_fixtures names them, a pad's sysfs device after an `=`.
def held(_hints):
    pads = []
    for entry in os.environ["CONTROLS_PADS"].split(","):
        name, _, device = entry.partition("=")
        pad = getattr(controls_fixtures, name)
        pads.append(Pad(**{**vars(pad), "device": device}) if device else pad)
    return pads


_gamepads.pads = held
