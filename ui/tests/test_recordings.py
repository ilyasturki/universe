import pytest
from PySide6.QtCore import Qt
from uitest import until

GAME, SESSION = "the-technomancer", "20260909-213045"


@pytest.fixture
def held_open(held, monkeypatch):
    """The session's recording is a held pipe, read by its player alone: its open lasts until the test calls the fixture."""
    from universe_ui.fake_core import FakeCore
    from universe_ui.screens.media import RecordingsList

    clip, extract = FakeCore._fake_clip, RecordingsList._extract

    def fake_clip(self, ident, session):
        out = clip(self, ident, session)
        return held.pipe(out) if session == SESSION else out

    def frames_of_files_only(self, job, frames, vaapi):
        if frames.path not in held.pipes.values():
            extract(self, job, frames, vaapi)

    monkeypatch.setattr(FakeCore, "_fake_clip", fake_clip)
    monkeypatch.setattr(RecordingsList, "_extract", frames_of_files_only)
    return held.release


def test_leaving_the_player_while_its_file_opens(held_open, api, look):
    from PySide6.QtMultimedia import QMediaPlayer
    from shiboken6 import isValid

    if look.stacked:
        api.screens.album.load(GAME)
        page = look.open("pages/PlayerPage.qml", {"gameId": GAME, "session": SESSION})
    else:
        page = look.open("pages/RecordingsPage.qml", {"game": api.allGames.byId(GAME), "session": SESSION})
        until(lambda: page.property("currentSession") == SESSION, "the list lands on the session")
        look.press(Qt.Key.Key_Return)
    player = until(lambda: (p := page.property("player")) is not None and p.property("source").toString() and p, "the player opens the file")
    assert player.mediaStatus() == QMediaPlayer.MediaStatus.LoadingMedia
    look.press(Qt.Key.Key_Escape, 1 if look.stacked else 2)
    until(lambda: not isValid(page), "B tears the player page down")
    assert isValid(player) and player.mediaStatus() == QMediaPlayer.MediaStatus.LoadingMedia, "the GUI went on while the player still opens its file"
    held_open()
    until(lambda: not isValid(player), "the player goes once its open settled")
