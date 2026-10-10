"""`just crawl`: every look walked by its keys on the fake core. Not a test_ file, so `just verify` leaves it out."""

import json
import os
from pathlib import Path

import pytest
from explorer import Explorer
from looks import LOOKS

from universe_ui.errors import UniverseError

OUT = Path(os.environ.get("UNIVERSE_CRAWL_OUT", ".crawl/run"))
PRESSES = int(os.environ.get("UNIVERSE_CRAWL_PRESSES", "3000"))
BUDGET_S = float(os.environ.get("UNIVERSE_CRAWL_BUDGET_S", "1800"))


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
    assert findings == []
