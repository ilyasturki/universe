import pytest
from looks import FocusTrace, Look, invoke, read, settle
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from uitest import until

from universe_ui.errors import UniverseError


@pytest.fixture
def upgraded(app, xdg, tmp_path):
    """A core on a fresh config whose last start ran 0.0.1: the fake changelog's 0.0.3 and 0.0.2 are new to it."""
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    core = FakeCore(FIXTURE, tmp_path / "core")
    (tmp_path / "core" / "state").mkdir(parents=True, exist_ok=True)
    (tmp_path / "core" / "state" / "last-version").write_text("0.0.1")
    client = CoreClient(core)
    yield client
    client.shutdown()


def versions(releases):
    return [r["version"] for r in releases]


def named(item, name):
    """Every item called `name` under `item`: a Repeater's delegates are its column's child items, not QObject children."""
    found = [item] if item.objectName() == name else []
    for child in item.childItems():
        found += named(child, name)
    return found


def test_a_release_s_media_offline_leaves_its_entry_text(api, fake, monkeypatch):
    changelog = api.screens.changelog
    media = changelog.releases[0]["sections"][0]["items"][0]["media"]
    errors = []
    fake.error.connect(lambda kind, message: errors.append(kind))

    def offline(url):
        raise UniverseError("Unavailable", "offline")

    monkeypatch.setattr(fake._core, "changelog_media", offline)
    changelog.fetch(media["url"])
    until(lambda: media["url"] not in changelog._asked, "the failure lands")
    monkeypatch.undo()
    assert changelog.files == {} and errors == [], "no file and no error: the entry reads as text"
    changelog.fetch(media["url"])
    until(lambda: changelog.files.get(media["url"]), "asked again once back online")


def test_the_game_defaults_switch_and_abouts_are_one(api, fake):
    form, changelog = api.screens.launch, api.screens.changelog
    form.load()
    at = until(lambda: next((i for i, r in enumerate(form.rows) if r["key"] == "desktop.whats_new"), None))
    assert form.rows[at]["value"] is True and not form.rows[at]["advanced"], "on by default, shown without Advanced"
    form.toggle(at)
    until(lambda: changelog.opensAfterUpdate is False, "About's switch follows")


def playing(page):
    """The page's clip once its player has opened the file."""
    from PySide6.QtMultimedia import QMediaPlayer

    def ready():
        clip = next(iter(named(page, "releaseClip")), None)
        player = clip.property("player") if clip is not None else None
        return player is not None and player.mediaStatus() == QMediaPlayer.MediaStatus.BufferedMedia and clip

    return until(ready, "the clip plays")


def under(look, name):
    """The Universe look's What's new holds the focus, and its page loads under it, often after it opened."""
    root = look.root
    dialog = look.find("whatsNew")
    until(lambda: dialog.property("activeFocus") is True)
    until(lambda: (page := root.property("activePage")) is not None and page.objectName() == name, "the page loads under it")
    assert dialog.property("activeFocus") is True, "the page landing under it leaves the focus in the dialog"


def over_the_page(look, name):
    """The Universe look's What's new: a dialog over the page it opened on, which stays the one shown under it."""
    root = look.root
    under(look, name)
    assert look.find("whatsNew").property("open") is True and root.property("subOpen") is False and root.property("returnLaunches") is None
    settle(look.window)
    look.trace = FocusTrace(look.window)


def back_on_the_page(look, name):
    root = look.root
    until(lambda: root.property("activePage").property("activeFocus") is True, "B gives the focus back to the page under it")
    settle(look.window)
    assert look.trace.stop().foreign("whatsNew", name) == [], "every frame of the way out: the focus in the dialog or on the page"
    assert look.find("whatsNew").property("open") is False


@pytest.fixture
def look(request, upgraded, rendered_looks, tmp_path):
    """A look started on the upgraded core."""
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    api = Api(upgraded, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    shown = Look(api, request.param)
    yield shown
    shown.close()
    api.shutdown()


@pytest.fixture
def held_open(rendered_looks, held, monkeypatch):
    """The clip's player reads its file through a held pipe, so its open lasts until the test calls the fixture."""
    from universe_ui.fake_core import FakeCore

    fetch = FakeCore.changelog_media

    def changelog_media(self, url):
        out = fetch(self, url)
        return held.pipe(out) if url.endswith(".mp4") else out

    monkeypatch.setattr(FakeCore, "changelog_media", changelog_media)
    return held.release


def test_b_while_the_clip_opens_closes_whats_new(held_open, look):
    from PySide6.QtMultimedia import QMediaPlayer
    from shiboken6 import isValid

    changelog = look.api.screens.changelog
    page = look.page("changelogPage")
    if look.name == "reprise":
        under(look, "homePage")
    clip = until(lambda: next((c for c in named(page, "releaseClip") if c.property("player") is not None), None), "the clip lands")
    player = clip.property("player")
    assert player.mediaStatus() == QMediaPlayer.MediaStatus.LoadingMedia
    look.press(Qt.Key.Key_Escape)
    until(lambda: changelog.pending == [], "B closes it, seen")
    until(lambda: not isValid(clip), "B tears the clip down")
    assert isValid(player) and player.mediaStatus() == QMediaPlayer.MediaStatus.LoadingMedia, "the GUI went on while the player still opens its file"
    held_open()
    until(lambda: not isValid(player), "the player goes once its open settled")


@pytest.mark.slow
def test_an_update_opens_whats_new_once_and_about_opens_the_changelog(look):
    changelog = look.api.screens.changelog
    page = look.page("changelogPage")
    until(lambda: page.property("fresh") is True, "the start opens what's new")
    assert versions(read(page, "releases")) == ["0.0.3", "0.0.2"]
    if look.name == "reprise":
        under(look, "homePage")
    shown = until(lambda: len(m := [i for i in named(page, "releaseMedia") if i.property("file")]) == 2 and m, "the highlights' media download")
    assert sorted(m.property("media")["kind"] for m in shown) == ["clip", "still"]
    still = until(lambda: next(iter(named(page, "releaseStill")), None), "the still is shown")
    clip = playing(page)
    files = changelog.files
    assert {still.property("source").toString(), clip.property("source").toString()} == set(files.values())
    if look.name == "reprise":
        over_the_page(look, "homePage")
        cover = look.root.property("activePage").property("menuAnchor")
        QTest.mouseClick(look.window, Qt.MouseButton.RightButton, Qt.KeyboardModifier.NoModifier, cover.mapToScene(cover.boundingRect().center()).toPoint())
        until(lambda: changelog.pending == [], "a right click, on the cover under it too, is B: it closes, seen")
        back_on_the_page(look, "homePage")
        assert look.root.property("menuOpen") is False, "the cover under the dialog never had the click"
    else:
        look.press(Qt.Key.Key_Escape)
        until(lambda: changelog.pending == [], "B closes it, seen")

    about = look.settings("about")
    until(lambda: about.property("sectionId") == "about")
    content = read(about, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    row = next(r for r in rows if r.get("key") == "changelog")
    settle(look.window)
    opening = FocusTrace(look.window)
    invoke(about, "activate", rows.index(row), row)
    page = until(lambda: (p := look.page("changelogPage")).property("fresh") is False and p, "About › What's new opens it")
    assert versions(read(page, "releases")) == ["0.0.3", "0.0.2", "0.0.1"]
    playing(page)
    if look.name == "reprise":
        over_the_page(look, "settingsPage")
        assert opening.stop().foreign("settingsPage", "whatsNew") == [], "every frame of the way in: the focus on the page or in the dialog"
    look.press(Qt.Key.Key_Escape)
    if look.name == "reprise":
        back_on_the_page(look, "settingsPage")

    about = look.settings("about")
    content = read(about, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    switch = next(r for r in rows if r.get("key") == "whats_new")
    assert switch["value"] is True, "About holds the switch, on"
    invoke(about, "activate", rows.index(switch), switch)
    until(lambda: changelog.opensAfterUpdate is False, "and turns it off")
    version = next(r for r in rows if r.get("key") == "version")
    invoke(about, "activate", rows.index(version), version)
    if look.name == "reprise":
        until(lambda: look.find("whatsNew").property("open") is True, "A on the version opens it too")
    page = until(lambda: (p := look.page("changelogPage")).property("fresh") is False and p, "A on the version opens it too")
    playing(page)
