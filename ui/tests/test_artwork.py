from uitest import record, settle, until


def slot_of(view, game, slot):
    return next(s for r in view.rows if r["id"] == game for s in r["slots"] if s["slot"] == slot)


def test_slots_carry_both_layers_and_a_pick_sits_over_the_default(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    slots = {s["slot"]: s for s in form.slots}
    assert list(slots) == ["box_front", "square", "banner", "background", "logo"]
    assert slots["box_front"]["kind"] == "default" and slots["box_front"]["hasDefault"]
    assert slots["box_front"]["origin"] == "steam" and slots["square"]["origin"] == "generated", "a keyless profile's art"
    assert not slots["box_front"]["hasOverride"]
    assert "?v=" in slots["box_front"]["url"]

    form.loadCandidates("box_front")
    settle(form)
    assert form.candidatesSlot == "box_front"
    assert [c["provider"] for c in form.candidates] == ["steam", "gamesdb", "gamesdb"]
    assert all(c["votes"] == 0 for c in form.candidates), "votes are SteamGridDB's"
    assert form.matched and not form.entryDiffers and not form.sgdbKey

    seen = []
    form.applied.connect(seen.append)
    changed = []
    fake.mediaChanged.connect(changed.append)
    form.apply("box_front", form.candidates[2]["url"])
    settle(form)
    assert seen == ["box_front"] and changed == ["dead-cells"]
    row = form.slot("box_front")
    assert row["kind"] == "picked" and row["hasOverride"] and row["hasDefault"]
    assert row["url"] == row["overrideUrl"] != row["defaultUrl"]
    assert api.library.get("dead-cells").assets.boxFront.toString() == row["overrideUrl"]

    assert form.removeOverride("box_front")
    row = form.slot("box_front")
    assert row["kind"] == "default" and row["url"] == row["defaultUrl"]
    assert not form.removeOverride("box_front")

    said = record(form.message)
    form.useFile("banner", fake.game("dead-cells")["media"]["logo"])
    settle(form)
    assert seen == ["box_front", "banner"] and changed[-1] == "dead-cells"
    assert form.slot("banner")["kind"] == "picked" and len(said) == 1


def test_search_and_pin_reload_the_candidates(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    form.loadCandidates("logo")
    form.search("")
    settle(form)
    hits = form.hits
    assert len(hits) == 3 and hits[0]["current"] and not hits[1]["current"]
    assert {h["provider"] for h in hits} == {"gamesdb"} and int(hits[0]["id"]) > 2**53, "GamesDB's ids reach QML whole, as text"
    form.pin(hits[1]["id"])
    settle(form)
    assert form.entryDiffers
    assert [h["current"] for h in form.hits] == [False, True, False]
    assert fake.game("dead-cells")["metadata"]["gamesdb_id"] == int(hits[1]["id"])
    assert form.candidatesSlot == "logo" and form.candidates


def test_a_steamgriddb_key_brings_its_art_and_its_entries(api, fake):
    form = api.screens.artwork
    form.load("dead-cells")
    form.loadCandidates("box_front")
    settle(form)
    assert not form.sgdbKey
    form.addKey("  abc123  ")
    settle(form)
    assert fake.config()["keys"]["sgdb"] == "abc123" and form.sgdbKey
    until(lambda: [c["provider"] for c in form.candidates][-1:] == ["sgdb"])
    assert [c["votes"] for c in form.candidates if c["provider"] == "sgdb"] == [3, 2, 1]
    form.search("")
    settle(form)
    assert {h["provider"] for h in form.hits} == {"sgdb"}
    form.pin(form.hits[2]["id"])
    settle(form)
    assert fake.game("dead-cells")["metadata"]["sgdb_id"] == int(form.hits[2]["id"])


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
    assert slot_of(view, "dead-cells", "banner")["kind"] == "picked"

    fake.core._game("control")["media"].pop("logo")
    view.load()
    settle(view)
    assert slot_of(view, "control", "logo")["kind"] == "missing" and view.missingGames == 1


def test_the_library_fetch_reports_its_progress_and_stops_after_the_game_in_hand(api, fake):
    view = api.screens.artworkOverview
    view.load()
    settle(view)
    said = record(view.message)
    view.refreshAll()
    assert view.job["ok"] is None and view.job["total"] == 0
    total = until(lambda: view.job["total"])
    assert total >= len(view.rows) and view.job["message"] != "", "progress names the game in hand"
    assert view.cancelRefresh() and view.job["cancelled"]
    assert not view.cancelRefresh(), "a second stop does nothing"
    until(lambda: view.job["ok"] is not None)
    assert view.job["ok"] is True and len(said) == 1
    assert view.job["done"] + 1 < total, "stopped well before the end"
