import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "bin"))


@pytest.fixture(autouse=True)
def home(tmp_path, monkeypatch):
    for name in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"):
        monkeypatch.setenv(name, str(tmp_path))
    monkeypatch.delenv("DOLPHIN_EMU_USERPATH", raising=False)
