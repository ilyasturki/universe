from uitest import until

from universe_ui.screens.search import word_score

SECTIONS = [
    {"id": "search", "label": "Search"},
    {"id": "launch", "label": "Launch"},
    {"id": "runners", "label": "Runners"},
    {"id": "controller", "label": "Controller"},
    {"id": "modules", "label": "Modules"},
    {"id": "themes", "label": "Themes"},
    {"id": "quit", "label": "Quit"},
]


def indexed(api):
    search = api.screens.search
    search.sections = SECTIONS
    search.load()
    until(lambda: search.ready)
    return search


def targets(search):
    return [r["target"] for r in search.results]


def target(page, ident="", key="", module=""):
    return {"page": page, "id": ident, "key": key, "module": module}


def test_word_scores():
    assert word_score("wayland", ["wayland"]) == 100 and word_score("way", ["wayland"]) == 90 and word_score("land", ["wayland"]) == 75
    assert word_score("wyland", ["wayland"]) == 60, "a typo away"
    assert word_score("wrk", ["working", "directory"]) == 35, "the letters in order from the word's start"
    assert word_score("hold", ["folders"]) == 0 and word_score("vrr", ["survives"]) == 0, "not across a word's middle"
    assert word_score("ab", ["cab"]) == 75 and word_score("abc", ["xyz"]) == 0


def test_the_index_spans_every_page(api, fake):
    search = indexed(api)
    assert search.ready and search.indexed > 100
    assert search.count == 0 and search.query == ""
    search.query = "wayland"
    assert targets(search) == [target("runner", "proton", "launch.wayland"), target("game", "", "launch.wayland")], "the global row, then the games collapsed"
    head = search.results[1]
    assert head["kind"] == "gamekey" and head["count"] == 6 and head["expanded"] is False and search.results[0]["advanced"] is False
    search.expand(1)
    rows = search.results
    assert rows[1]["expanded"] is True and len(rows) == 2 + head["count"] and all(r["kind"] == "gamerow" for r in rows[2:])
    assert rows[2]["target"] == target("game", "the-technomancer", "launch.wayland") and rows[2]["inherited"] is True
    assert all(r["image"].startswith("file://") for r in rows[2:] if r["image"]), "the game's art on its row"
    search.expand(1)
    assert len(search.results) == 2 and search.results[1]["expanded"] is False


def test_synonyms_descriptions_values_and_typos(api, fake):
    search = indexed(api)
    search.query = "vrr"
    assert targets(search)[0] == target("launch", "", "launch.gamescope_adaptive_sync"), "a synonym"
    search.query = "eventfd"
    assert targets(search)[:2] == [target("runner", "proton", "launch.esync"), target("runner", "wine", "launch.esync")], "a word of the description"
    assert search.results[0]["advanced"] is True
    search.query = "av1 10-bit"
    assert targets(search)[0] == target("module", "capture", "codec", "capture"), "the current value, as its label reads"
    search.query = "h265"
    codec = next(s for m in fake.modules() if m["id"] == "capture" for s in m["settings"] if s["key"] == "codec")
    assert "h265" in codec["keywords"] and targets(search)[0] == target("module", "capture", "codec", "capture"), "a module setting's own keywords"
    search.query = "wyland"
    assert [t["key"] for t in targets(search)] == ["launch.wayland", "launch.wayland"], "a typo"
    search.query = "hud"
    assert [t["key"] for t in targets(search)][:2] == ["launch.mangohud", "launch.mangohud"]
    search.query = "quit"
    assert search.results[0]["kind"] == "section" and targets(search)[0] == target("section", "quit"), "a section wins a tie"
    search.query = "hold"
    assert targets(search)[0] == target("controller", "", "controller.hold_ms")
    search.query = "gsr"
    assert any(r["target"] == target("module", "capture", "gsr_extra_args", "capture") and r["advanced"] for r in search.results), "a config-only setting"
    search.query = "dolphin"
    assert targets(search)[0] == target("runner", "dolphin")
    search.query = "switch 2"
    assert targets(search)[0] == target("themes", "switch2", "theme")
    search.query = "intro"
    assert targets(search)[0] == target("themes", "", "boot_intro")
    search.query = "zzzz"
    assert search.count == 0


def test_a_game_in_the_query_narrows_to_it(api, fake):
    search = indexed(api)
    search.query = "technomancer wayland"
    assert targets(search) == [target("game", "the-technomancer", "launch.wayland")]
    search.query = "technomancer"
    assert search.results[0]["kind"] == "game" and targets(search)[0] == target("game", "the-technomancer")
    assert [t["key"] for t in targets(search)[1:4]] == ["launch.gamescope", "launch.gamescope_resolution", "launch.gamescope_refresh"], (
        "then every setting of the game, in page order"
    )
    assert all(t["id"] == "the-technomancer" for t in targets(search)[1:])
    search.query = "technomancer cursor"
    assert target("game", "the-technomancer", "cursor", "capture") in targets(search), "a module's game setting"
    assert target("game", "the-technomancer", "desktop.hide_cursor") in targets(search)


def test_the_index_follows_the_config(api, fake):
    search = indexed(api)
    search.query = "av1 10-bit"
    codec = target("module", "capture", "codec", "capture")
    assert codec in targets(search)
    fake.setSetting("capture", "", "codec", "h264")
    search.load()
    until(lambda: codec not in targets(search), "a reload reads the values again and keeps the query")
