"""`just crawl`: every look walked by its keys on the fake core. Not a test_ file, so `just verify` leaves it out."""

import json
import os
from pathlib import Path

import pytest
from explorer import Explorer
from looks import LOOKS

OUT = Path(os.environ.get("UNIVERSE_CRAWL_OUT", ".crawl/run"))
BUDGET_S = float(os.environ.get("UNIVERSE_CRAWL_BUDGET_S", "900"))


@pytest.mark.slow
@pytest.mark.parametrize("name", LOOKS)
def test_crawl(api, fake, qtlog, name):
    seen = [0]

    def warnings():
        records = [r for r in qtlog.records if r.type_name in ("QtWarningMsg", "QtCriticalMsg", "QtFatalMsg") and not r.ignored]
        new = records[seen[0] :]
        seen[0] = len(records)
        return [r.message for r in new]

    explorer = Explorer(api, fake, name, OUT / name, budget_s=BUDGET_S, log=warnings)
    findings = explorer.crawl()
    print(f"\n{name}: {len(explorer.states)} states, {explorer.steps} presses, {explorer.resets} resets, complete={explorer.complete}")
    for f in findings:
        print(f"  {f['kind']:16} {f['page']:24} {' '.join(f['path'])[:60]:60} {f['detail'][:120]}")
    (OUT / name / "findings.json").write_text(json.dumps(findings, indent=1))
    assert findings == []
