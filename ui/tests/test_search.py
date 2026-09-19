from conftest import wait_for
from universe_ui.screens.search import word_score

SECTIONS = [{"id": "search", "label": "Search"}, {"id": "launch", "label": "Launch"}, {"id": "runners", "label": "Runners"}, {"id": "controller", "label": "Controller"},
            {"id": "modules", "label": "Modules"}, {"id": "themes", "label": "Themes"}, {"id": "quit", "label": "Quit"}]


def indexed(api):
    search = api.screens.search
    search.sections = SECTIONS
    search.load()
    if not search.ready:
        wait_for(search.readyChanged, 5000)
    return search


def labels(search):
    return [(r["path"], r["label"], r["display"]) for r in search.results]


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
    assert labels(search) == [("Runners › Proton", "Wayland", "On"), ("Games › Proton", "Wayland", "in 6 games")], "the global row, then the games collapsed"
    head = search.results[1]
    assert head["kind"] == "gamekey" and head["expanded"] is False and head["target"] == {"page": "game", "id": "", "key": "launch.wayland", "module": ""}
    assert search.results[0]["target"] == {"page": "runner", "id": "proton", "key": "launch.wayland", "module": ""} and search.results[0]["advanced"] is False
    search.expand(1)
    rows = search.results
    assert rows[1]["expanded"] is True and len(rows) == 8 and rows[2]["kind"] == "gamerow" and rows[2]["label"] == "The Technomancer"
    assert rows[2]["target"] == {"page": "game", "id": "the-technomancer", "key": "launch.wayland", "module": ""} and rows[2]["inherited"] is True
    assert all(r["image"].startswith("file://") for r in rows[2:] if r["image"]), "the game's art on its row"
    search.expand(1)
    assert len(search.results) == 2 and search.results[1]["expanded"] is False


def test_synonyms_descriptions_values_and_typos(api, fake):
    search = indexed(api)
    search.query = "vrr"
    assert labels(search)[0] == ("Launch › Display", "Adaptive sync", "Off"), "a synonym"
    search.query = "eventfd"
    assert [r["label"] for r in search.results][:2] == ["Esync", "Esync"] and search.results[0]["path"] == "Runners › Proton › Sync", "a word of the description"
    assert search.results[0]["advanced"] is True and search.results[0]["tag"] == "ADVANCED" and search.results[0]["detail"].startswith("Faster thread")
    search.query = "av1_10bit"
    assert labels(search)[0] == ("Modules › Video capture", "Video codec", "av1_10bit"), "the current value"
    search.query = "wyland"
    assert [r["label"] for r in search.results] == ["Wayland", "Wayland"], "a typo"
    search.query = "hud"
    assert [r["label"] for r in search.results][:2] == ["MangoHud", "MangoHud"]
    search.query = "quit"
    assert search.results[0]["kind"] == "section" and search.results[0]["target"] == {"page": "section", "id": "quit", "key": "", "module": ""}, "a section wins a tie"
    search.query = "hold"
    assert search.results[0]["label"] == "Hold length (ms)" and search.results[0]["target"] == {"page": "controller", "id": "", "key": "controller.hold_ms", "module": ""}
    search.query = "gsr"
    assert any(r["target"] == {"page": "module", "id": "capture", "key": "gsr_extra_args", "module": "capture"} and r["advanced"] for r in search.results), "a config-only setting"
    search.query = "dolphin"
    assert search.results[0]["target"] == {"page": "runner", "id": "dolphin", "key": "", "module": ""} and search.results[0]["image"] == "assets/runners/dolphin.svg"
    search.query = "switch 2"
    assert search.results[0]["target"] == {"page": "themes", "id": "switch2", "key": "theme", "module": ""}
    search.query = "zzzz"
    assert search.count == 0


def test_a_game_in_the_query_narrows_to_it(api, fake):
    search = indexed(api)
    search.query = "technomancer wayland"
    assert labels(search) == [("The Technomancer", "Wayland", "On")]
    assert search.results[0]["target"] == {"page": "game", "id": "the-technomancer", "key": "launch.wayland", "module": ""}
    search.query = "technomancer"
    assert search.results[0]["kind"] == "game" and search.results[0]["target"] == {"page": "game", "id": "the-technomancer", "key": "", "module": ""}
    assert [r["label"] for r in search.results[1:4]] == ["Gamescope", "Resolution", "Refresh rate"], "then every setting of the game, in page order"
    assert all(r["path"] == "The Technomancer" for r in search.results[1:])
    search.query = "technomancer cursor"
    rows = {r["label"]: r for r in search.results}
    assert rows["Show the cursor in the recording"]["target"] == {"page": "game", "id": "the-technomancer", "key": "cursor", "module": "capture"}, "a module's game setting"
    assert rows["Hide the cursor while playing"]["target"]["key"] == "desktop.hide_cursor"


def test_the_index_follows_the_config(api, fake):
    search = indexed(api)
    search.query = "wayland"
    assert search.results[0]["display"] == "On"
    fake.setConfig("launch.wayland", "false")
    search.load()
    wait_for(search.readyChanged, 5000)
    assert search.results[0]["display"] == "Off", "a reload reads the values again and keeps the query"
