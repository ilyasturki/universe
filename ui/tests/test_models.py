from PySide6.QtCore import QDateTime, QUrl

from universe_ui.models import (
    FavouriteGames,
    Game,
    LibraryGames,
    LimitedGames,
    RecentGames,
    SearchGames,
    SortedGames,
    sort_title,
)


def titles(proxy):
    return [proxy.get(i).title for i in range(proxy.count)]


def test_game_defaults_when_keys_missing(app):
    game = Game({}, None)
    assert game.id == ""
    assert game.title == ""
    assert game.playTime == 0
    assert game.playCount == 0
    assert game.lastPlayed is None
    assert game.releaseYear == 0
    assert game.developerList == []
    assert game.genreList == []
    assert game.players == 1
    assert game.favorite is False
    assert game.hidden is False
    assert game.assets.boxFront.isEmpty()
    assert game.assets.screenshotList == []


def test_sort_title_drops_leading_article(app):
    assert sort_title("The Witcher 3") == "Witcher 3"
    assert sort_title("A Hat in Time") == "Hat in Time"
    assert sort_title("An Untitled Story") == "Untitled Story"
    assert sort_title("Theatrhythm") == "Theatrhythm"
    assert sort_title("The") == "The"
    assert Game({"title": "The Witcher 3"}, None).sortTitle == "Witcher 3"
    assert Game({"title": "The Witcher 3", "sort_title": "Witcher"}, None).sortTitle == "Witcher"


def test_game_decodes_variants(app):
    game = Game({
        "id": "x", "title": "X", "favorite": True,
        "stats": {"hours": 1.5, "play_count": "3", "last_played": "2026-09-09T22:41:00+02:00"},
        "metadata": {"developer": "A, B", "genres": ["Action"], "release_year": "2016",
                     "extra": {"metacritic": 61}},
        "media": {"box_front": "/tmp/box.png", "square": "/tmp/sq.png", "screenshots": ["/tmp/a.png", "http://x/b.png"]},
        "tags": "rpg, sci-fi",
    }, None)
    assert game.playTime == 5400
    assert game.playCount == 3
    assert isinstance(game.lastPlayed, QDateTime) and game.lastPlayed.isValid()
    assert game.developerList == ["A", "B"]
    assert game.genreList == ["Action"]
    assert game.releaseYear == 2016
    assert game.extra == {"metacritic": [61]}
    assert game.tags == ["rpg", "sci-fi"]
    assert game.assets.boxFront.isLocalFile() and game.assets.square.isLocalFile()
    assert [u.toString(QUrl.FormattingOptions(QUrl.UrlFormattingOption.RemoveQuery)) for u in game.assets.screenshotList] == ["file:///tmp/a.png", "http://x/b.png"]


def test_game_decodes_daemon_shapes(app):
    game = Game({
        "id": "y", "title": "Y", "platform": "Nintendo Switch", "release_year": 2017,
        "source": {"kind": "lutris", "lutris_slug": "y", "gog_id": ""},
        "metadata": {"developers": ["N"], "metacritic": 97, "players": 0, "rawg_id": 0, "sgdb_id": 12, "hltb_main": 50},
    }, None)
    assert game.source == "lutris"
    assert game.platform == "Nintendo Switch"
    assert game.releaseYear == 2017
    assert game.players == 1
    assert game.extra == {"metacritic": [97], "hltb-main": [50]}


def test_library_excludes_hidden_and_groups_by_source(api):
    games = api.allGames
    assert games.count == 8
    assert games.byId("cyberpunk-2077") is None
    assert games.byId("the-technomancer").title == "The Technomancer"
    collections = api.collections
    assert [collections.get(i).name for i in range(collections.count)] == ["Nintendo Switch", "Nintendo Wii", "Windows"]
    assert [collections.get(i).shortName for i in range(collections.count)] == ["switch", "wii", "windows"]
    assert [collections.get(i).games.count for i in range(collections.count)] == [1, 1, 6]
    assert games.byId("dishonored").collections.get(0).shortName == "windows"
    assert games.byId("dishonored").source == "lutris"


def test_recent_games(api):
    recent = RecentGames()
    recent.setSourceModel(api.allGames)
    assert recent.count == 6
    assert titles(recent)[:3] == ["The Technomancer", "Mini Metro", "Dead Cells"]
    assert "Mirror's Edge" not in titles(recent)
    recent.playingId = api.allGames.byId("mirrors-edge").id
    assert titles(recent)[:2] == ["Mirror's Edge", "The Technomancer"] and recent.count == 7
    recent.playingId = "dead-cells"
    assert titles(recent)[:2] == ["Dead Cells", "The Technomancer"] and recent.count == 6
    recent.playingId = ""
    assert titles(recent)[:3] == ["The Technomancer", "Mini Metro", "Dead Cells"]


def test_sorted_and_limited(api):
    by_release = SortedGames()
    by_release.setSourceModel(api.allGames)
    by_release.sortRoleName = "releaseYear"
    by_release.descending = True
    assert by_release.get(0).title == "Control"
    assert set(titles(by_release)[-2:]) == {"Mirror's Edge", "LEGO Batman: The Videogame"}

    newest = LimitedGames()
    newest.setSourceModel(by_release)
    newest.limit = 3
    assert titles(newest) == titles(by_release)[:3]
    newest.limit = 1
    assert newest.count == 1


def test_favourites_with_pinned(api):
    favourites = FavouriteGames()
    favourites.setSourceModel(api.allGames)
    assert titles(favourites) == ["The Technomancer", "Dead Cells"]
    row = api.allGames.rowOf(api.allGames.byId("control"))
    favourites.pinned = [row]
    assert titles(favourites) == ["The Technomancer", "Dead Cells", "Control"]
    assert favourites.sourceRow(2) == row


def test_search(api):
    matches = SearchGames()
    matches.setSourceModel(api.allGames)
    matches.query = "METRO"
    assert titles(matches) == ["Mini Metro"]
    matches.query = ""
    assert matches.count == 8


def test_library_sort_modes(api):
    library = LibraryGames()
    library.setSourceModel(api.allGames)
    library.sortMode = 0
    assert set(titles(library)[-2:]) == {"Mirror's Edge", "Batman: Arkham Origins"}
    assert library.get(0).title == "The Technomancer"
    library.sortMode = 1
    assert titles(library) == sorted(titles(library), key=lambda t: sort_title(t).casefold())
    library.sortMode = 2
    assert library.get(0).title == "The Technomancer"


def test_favorite_setter_writes_through(api, fake):
    game = api.allGames.byId("control")
    assert game.favorite is False
    game.favorite = True
    assert fake.game("control")["favorite"] is True
    assert api.allGames.byId("control").favorite is True
    favourites = FavouriteGames()
    favourites.setSourceModel(api.allGames)
    assert "Control" in titles(favourites)
