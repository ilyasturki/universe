import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bin"))
import _gamepads
import _mapping
import controls_fixtures
from _gamepads import Pad


# CONTROLS_PADS: SDL's list, as controls_fixtures names them, a pad's sysfs device after an `=`; CONTROLS_HINTS gets the hints SDL was started with.
def held(hints):
    if os.environ.get("CONTROLS_HINTS"):
        Path(os.environ["CONTROLS_HINTS"]).write_text(json.dumps(hints))
    pads = []
    for entry in filter(None, os.environ["CONTROLS_PADS"].split(",")):
        name, _, device = entry.partition("=")
        pad = getattr(controls_fixtures, name)
        pads.append(Pad(**{**vars(pad), "device": device}) if device else pad)
    return pads


_gamepads.pads = held
# CONTROLS_DEVICES: what `universe --json controller ls` lists, the machine's own pads left out.
_mapping.devices = lambda: json.loads(os.environ.get("CONTROLS_DEVICES") or "[]")
