"""The artwork screens against the fake: slots with their layers, candidates, picks, the pin flow, the overview."""

from conftest import wait_for


def test_slots_carry_both_layers_and_a_pick_sits_over_the_default(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    slots = {s["slot"]: s for s in form.slots}
    assert list(slots) == ["box_front", "square", "tile", "background", "logo"]
    assert slots["box_front"]["kind"] == "fetched" and slots["box_front"]["hasDefault"]
    assert not slots["box_front"]["hasOverride"]
    assert "?v=" in slots["box_front"]["url"]

    form.loadCandidates("box_front")
    assert form.candidatesSlot == "box_front"
    assert len(form.candidates) == 6 and form.candidates[0]["votes"] == 6

    seen = []
    form.applied.connect(seen.append)
    changed = []
    fake.mediaChanged.connect(changed.append)
    form.apply("box_front", form.candidates[2]["url"])
    assert seen == ["box_front"] and changed == ["dead-cells"]
    row = form.slot("box_front")
    assert row["kind"] == "picked" and row["hasOverride"] and row["hasDefault"]
    assert row["url"] == row["overrideUrl"] != row["defaultUrl"]
    # The library shows the pick.
    assert api.library.get("dead-cells").assets.boxFront.toString() == row["overrideUrl"]

    assert form.removeOverride("box_front")
    row = form.slot("box_front")
    assert row["kind"] == "fetched" and row["url"] == row["defaultUrl"]
    assert not form.removeOverride("box_front")


def test_search_and_pin_reload_the_candidates(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    form.loadCandidates("logo")
    form.search("")
    hits = form.hits
    assert len(hits) == 3 and hits[0]["current"] and not hits[1]["current"]
    messages = []
    form.message.connect(messages.append)
    form.pin(hits[1]["id"])
    assert form.sgdbId == hits[1]["id"]
    assert [h["current"] for h in form.hits] == [False, True, False]
    assert fake.game("dead-cells")["metadata"]["sgdb_id"] == str(hits[1]["id"])
    assert form.candidatesSlot == "logo" and form.candidates
    assert "Remastered" in messages[-1]


def test_overview_filters_one_slot_across_the_library(api, fake):
    view = api.screens.artworkOverview
    view.load()
    assert not view.busy
    titles = [t["title"] for t in view.tiles]
    assert titles == sorted(titles, key=str.casefold)
    assert all(t["kind"] == "fetched" for t in view.tiles)
    counts = view.counts
    assert counts["all"] == len(view.tiles) and counts["missing"] == 0

    fake.mediaSetSlot("dead-cells", "tile", fake.game("dead-cells")["media"]["logo"])
    # A pick reloads the overview after a short debounce; the test does it by hand.
    view.load()
    view.slot = "tile"
    view.filter = "picked"
    assert [t["id"] for t in view.tiles] == ["dead-cells"]
    assert view.counts["picked"] == 1
    view.filter = "missing"
    assert view.tiles == []
