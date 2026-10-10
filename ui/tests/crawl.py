"""`just crawl`: every look walked by its keys on the fake core. Not a test_ file, so `just verify` leaves it out."""

import json
import os
from pathlib import Path

import pytest
from explorer import Explorer
from looks import LOOKS

from universe_ui.errors import UniverseError

OUT = Path(os.environ.get("UNIVERSE_CRAWL_OUT", ".crawl/run"))
PRESSES = int(os.environ.get("UNIVERSE_CRAWL_PRESSES", "6000"))
BUDGET_S = float(os.environ.get("UNIVERSE_CRAWL_BUDGET_S", "3600"))

# What the crawl found and its owners have yet to settle, each (kind, page or the detail's start) with why it stands:
# a finding not listed here fails the crawl. Take a line out once its fix lands.
KNOWN = {
    "reprise": {
        # The action menu is 460 dp wide: a build's row loses its size and where it came from.
        ("truncated", "Use GE-Proton"),
        ("truncated", "Remove GE-Proton"),
        ("truncated", "Follow the newest GE-Proton"),
    },
    "switch2": {
        # Settings' sidebar cuts its sections' second lines at 1280×720.
        ("truncated", "Headphones, speakers and keyboards"),
        ("truncated", "Saves, prefixes and every folder Universe writes to"),
        ("truncated", "Every setting, by name, purpose or value"),
        ("truncated", "Checks that the internet answers through this connection."),
        ("truncated", "The PS5 home screen"),
        # The controller page's help lines, the artwork slot's key hint, the install page's title and progress, a screen's mode.
        ("truncated", "Hold its pairing button until the lights blink"),
        ("truncated", "A press this long is a hold"),
        ("truncated", "Add a SteamGridDB key for more art"),
        ("truncated", "The Witcher 3: Wild Hunt"),
        ("truncated", "Paused · "),
        ("truncated", "DP-1 2560×1440 @ 144 Hz"),
    },
    "ps5": {
        # The session log's process column.
        ("truncated", "pressure-vessel-wrap"),
    },
}


@pytest.mark.slow
@pytest.mark.parametrize("name", LOOKS)
def test_crawl(api, fake, qtlog, monkeypatch, name):
    def refused(ident, screen, splash=""):
        raise UniverseError("Busy", "the explorer stops at a launch")

    # A played game would reorder Home for the rest of the walk, and for the next run's shots.
    monkeypatch.setattr(fake.core, "launch", refused)
    seen = [0]

    def warnings():
        records = [r for r in qtlog.records if r.type_name in ("QtWarningMsg", "QtCriticalMsg", "QtFatalMsg") and not r.ignored]
        new = records[seen[0] :]
        seen[0] = len(records)
        return [r.message for r in new]

    explorer = Explorer(api, fake, name, OUT / name, presses=PRESSES, budget_s=BUDGET_S, log=warnings)
    findings = explorer.crawl()
    print(f"\n{name}: {len(explorer.states)} states, {explorer.steps} presses, {explorer.resets} resets, complete={explorer.complete}")
    for f in findings:
        print(f"  {f['kind']:16} {f['page']:24} {' '.join(f['path'])[:60]:60} {f['detail'][:120]}")
    (OUT / name / "findings.json").write_text(json.dumps(findings, indent=1))
    assert [f for f in findings if not known(name, f)] == []


def known(look, finding):
    return any(finding["kind"] == kind and (finding["page"] == what or finding["detail"].startswith(what)) for kind, what in KNOWN[look])
