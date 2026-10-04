import json
import os

from looks import invoke
from PySide6.QtCore import QObject, Qt

from conftest import record, until


def test_the_store_orders_unlocks_first_and_folds_the_hidden_ones(api, fake):
    store = api.screens.achievements
    store.load("batman-arkham-origins")
    assert store.loading
    until(lambda: not store.loading, "the read runs off the UI thread")
    rows = store.rows
    assert (store.total, store.unlocked, store.loading) == (6, 3, False)
    assert [r["key"] for r in rows] == ["detective", "blackgate", "rooftops", "combo", "iam", "hidden"], "newest unlock first, then the most common locked one"
    assert "22:18" in rows[0]["dateText"] and rows[0]["rarityText"].startswith("19%"), "local time; a common one rounded"
    assert rows[4]["rarityText"].startswith("2.1%"), "a rare one to the tenth"
    folded = rows[-1]
    assert folded["hidden"] == 1 and folded["icon"] == "" and folded["rarity"] < 0 and "Freeze" not in folded["description"]
    fetched = store.fetchedText
    assert fetched

    store.refresh()
    assert store.loading
    until(lambda: not store.loading)
    assert store.fetchedText != fetched, "asked the store again"


def test_every_hidden_locked_one_folds_into_one_last_row():
    from universe_ui.screens.achievements import _order, _row

    items = [
        {"key": "a", "unlocked_at": "2026-09-01T00:00:00+00:00", "hidden": True},
        {"key": "b", "hidden": True, "icon_locked": "https://store/b.png", "rarity": 50},
        {"key": "c", "rarity": 10},
        {"key": "d", "hidden": True, "rarity": 1},
    ]
    rows = _order([_row(i) for i in items])
    assert [r["key"] for r in rows] == ["a", "c", "hidden"], "a hidden one found is a row like the others"
    assert rows[-1]["hidden"] == 2 and rows[-1]["icon"] == ""


def test_a_game_no_source_lists_says_so(api, fake):
    store = api.screens.achievements
    store.load("dishonored")
    until(lambda: not store.loading)
    assert store.count == 0 and "no source lists" in store.error


def test_the_game_model_carries_the_counts(api, fake):
    batman, metro = api.allGames.byId("batman-arkham-origins"), api.allGames.byId("mini-metro")
    assert (batman.achievementsTotal, batman.achievementsUnlocked) == (6, 3)
    assert metro.achievementsTotal == 0


def test_game_settings_carry_the_sources_own_switch(api, fake):
    form = api.screens.gameSettings
    form.load("batman-arkham-origins")
    row = next(r for r in form.rows if r.get("key") == "sources.gog.achievements")
    assert row["value"] is True and row["origin"] == "default"
    fake.set("batman-arkham-origins", "sources.gog.achievements", "false")
    assert fake.core.source_settings("gog", "batman-arkham-origins")["achievements"] is False
    assert fake.core.source_settings("gog")["achievements"] is True, "the global value stands"
    form.load("dishonored")
    assert not any(str(r.get("key", "")).startswith("sources.") for r in form.rows), "a Lutris game has no GOG switch"


def test_an_unlock_mid_session_reaches_home_once_and_a_replay_shows_the_stamped_ones_again(api, fake):
    cache = fake.core._data["achievements"]["batman-arkham-origins"]
    path = fake.core._game_dir("batman-arkham-origins") / "achievements.json"

    def stamp(at, keys):
        cache["replay"] = {"at": at, "keys": keys}
        path.with_suffix(".tmp").write_text(json.dumps(cache))
        os.replace(path.with_suffix(".tmp"), path)

    stamp("2026-09-01T10:00:00+00:00", ["detective"])
    seen = record(api.home.achievementUnlocked)
    fake.launch("batman-arkham-origins", "DP-1")
    item = until(lambda: seen, "the fake source files one in once the session runs")[0][0]
    assert (item["gameId"], item["key"], item["name"]) == ("batman-arkham-origins", "combo", "Unbreakable")
    until(lambda: api.allGames.byId("batman-arkham-origins").achievementsUnlocked == 4)
    stamp("2026-09-27T12:00:00+00:00", ["rooftops", "detective"])
    until(lambda: len(seen) >= 3)
    assert [i["key"] for (i,) in seen] == ["combo", "rooftops", "detective"], (
        "the unlocks from before the session and the stamp from before this run of the UI are no news, the unlock comes once; "
        "then each stamped key, known or not, in the stamp's order"
    )
    assert seen[-1][0]["name"] and seen[-1][0]["gameId"] == "batman-arkham-origins"


def test_each_looks_page_ends_on_the_folded_row_and_home_end_reach_either_end(api, look):
    invoke(look.root, "push" if look.stacked else "openSub", "pages/AchievementsPage.qml", look.game("batman-arkham-origins"))
    page = look.page("achievements" if look.name == "ps5" else "achievementsPage")
    cursor = page.findChild(QObject, "achievements") or page
    store = api.screens.achievements
    until(lambda: not store.loading and store.count == 6)
    look.press(Qt.Key.Key_End)
    until(lambda: cursor.property("index") == store.count - 1 and store.rows[-1]["key"] == "hidden", "End reaches the folded row")
    look.press(Qt.Key.Key_Home)
    until(lambda: cursor.property("index") == 0, "Home the first one")
