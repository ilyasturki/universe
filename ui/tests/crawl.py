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

# What the crawl found and its owners have yet to settle, each (kind, page or the detail's start): a finding not listed
# here fails the crawl. Take a line out once its fix lands. A page the walk's presses ran out before is told, not failed.
KNOWN = {
    "reprise": {
        # Seen once: Artwork's keyboard sheet closed and kept the focus, hidden; not reproduced by hand yet.
        ("focus-unseen", "artworkPage"),
    },
    "switch2": set(),
    "ps5": {
        # The PS5 look opens its setup on a first start with an empty library only.
        ("unreached", "onboardingPage"),
    },
}


@pytest.mark.slow
@pytest.mark.parametrize("name", LOOKS)
def test_crawl(api, fake, qtlog, monkeypatch, name):
    def refused(*_):
        raise UniverseError("Busy", "the explorer leaves the library as it found it")

    def set_but_order(ident, key, value, original=fake.core.set):
        if key in ("favourite", "hidden"):
            refused()
        return original(ident, key, value)

    # A game played, favoured, hidden or removed would reorder Home for the rest of the walk, and for the next run's shots.
    for write in ("launch", "remove", "uninstall", "remove_screenshot", "remove_recording", "remove_journal_entry"):
        monkeypatch.setattr(fake.core, write, refused)
    monkeypatch.setattr(fake.core, "set", set_but_order)
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
    assert [f for f in findings if f["kind"] != "unexplored" and not known(name, f)] == []


def known(look, finding):
    return any(finding["kind"] == kind and (finding["page"] == what or finding["detail"].startswith(what)) for kind, what in KNOWN[look])
