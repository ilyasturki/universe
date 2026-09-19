from conftest import settle


def test_slots_carry_both_layers_and_a_pick_sits_over_the_default(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    slots = {s["slot"]: s for s in form.slots}
    assert list(slots) == ["box_front", "square", "banner", "background", "logo"]
    assert slots["box_front"]["kind"] == "default" and slots["box_front"]["hasDefault"]
    assert slots["box_front"]["kindLabel"] == "SteamGridDB", "one label carries the state and the origin"
    assert slots["square"]["use"] == "Home rail, Switch 2 tiles"
    assert not slots["box_front"]["hasOverride"]
    assert "?v=" in slots["box_front"]["url"]

    form.loadCandidates("box_front")
    settle(form)
    assert form.candidatesSlot == "box_front"
    assert len(form.candidates) == 6 and form.candidates[0]["votes"] == 6
    assert form.entry == "Dead Cells (2016)" and not form.entryDiffers

    seen = []
    form.applied.connect(seen.append)
    changed = []
    fake.mediaChanged.connect(changed.append)
    form.apply("box_front", form.candidates[2]["url"])
    settle(form)
    assert seen == ["box_front"] and changed == ["dead-cells"]
    row = form.slot("box_front")
    assert row["kind"] == "picked" and row["kindLabel"] == "Your pick" and row["hasOverride"] and row["hasDefault"]
    assert row["url"] == row["overrideUrl"] != row["defaultUrl"]
    assert api.library.get("dead-cells").assets.boxFront.toString() == row["overrideUrl"]

    assert form.removeOverride("box_front")
    row = form.slot("box_front")
    assert row["kind"] == "default" and row["url"] == row["defaultUrl"]
    assert not form.removeOverride("box_front")

    messages = []
    form.message.connect(messages.append)
    form.useFile("banner", fake.game("dead-cells")["media"]["logo"])
    settle(form)
    assert seen == ["box_front", "banner"] and changed[-1] == "dead-cells"
    assert form.slot("banner")["kind"] == "picked" and messages[-1].startswith("Banner picked for Dead Cells: ")


def test_search_and_pin_reload_the_candidates(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    form.loadCandidates("logo")
    form.search("")
    settle(form)
    hits = form.hits
    assert len(hits) == 3 and hits[0]["current"] and not hits[1]["current"]
    messages = []
    form.message.connect(messages.append)
    form.pin(hits[1]["id"])
    settle(form)
    assert form.sgdbId == hits[1]["id"]
    assert form.entry == "Dead Cells Remastered (2021)" and form.entryDiffers
    assert [h["current"] for h in form.hits] == [False, True, False]
    assert fake.game("dead-cells")["metadata"]["sgdb_id"] == str(hits[1]["id"])
    assert form.candidatesSlot == "logo" and form.candidates
    assert "Remastered" in messages[-1]


def test_overview_lays_the_library_out_as_games_by_slots(api, fake):
    view = api.screens.artworkOverview
    view.load()
    settle(view)
    titles = [r["title"] for r in view.rows]
    assert titles == sorted(titles, key=str.casefold)
    assert [c["slot"] for c in view.columns] == ["box_front", "square", "banner", "background", "logo"]
    assert all([s["slot"] for s in r["slots"]] == [c["slot"] for c in view.columns] for r in view.rows), "every row in column order"
    assert all(s["kind"] == "default" for r in view.rows for s in r["slots"])

    fake.mediaSetSlot("dead-cells", "banner", fake.game("dead-cells")["media"]["logo"])
    view.load()
    settle(view)
    row = next(r for r in view.rows if r["id"] == "dead-cells")
    assert row["slots"][2]["kind"] == "picked" and row["slots"][2]["kindLabel"] == "Your pick"

    fake.core._game("control")["media"].pop("logo")
    view.load()
    settle(view)
    row = next(r for r in view.rows if r["id"] == "control")
    assert row["slots"][4]["kind"] == "missing" and row["slots"][4]["kindLabel"] == "Missing"
