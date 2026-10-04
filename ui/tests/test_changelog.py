import pytest
from looks import Look, invoke, read
from PySide6.QtCore import Qt
from uitest import until


@pytest.fixture
def upgraded(app, xdg, tmp_path):
    """A core whose last start ran 0.0.1, the what's-new page on: the fake changelog's 0.0.3 and 0.0.2 are new to it."""
    from universe_ui.fake_core import FIXTURE, FakeCore
    from universe_ui.universe_client import CoreClient

    core = FakeCore(FIXTURE, tmp_path / "core")
    core.set_setting("desktop.whats_new", "true")
    (tmp_path / "core" / "state").mkdir(parents=True, exist_ok=True)
    (tmp_path / "core" / "state" / "last-version").write_text("0.0.1")
    client = CoreClient(core)
    yield client
    client.shutdown()


def versions(releases):
    return [r["version"] for r in releases]


def test_whats_new_comes_once_after_an_update_and_only_when_turned_on(fake, tmp_path):
    from universe_ui.screens.changelog import Changelog

    recorded = tmp_path / "core" / "state" / "last-version"
    assert Changelog(fake).pending == [] and recorded.read_text() == "0.0.3", "a first start records the version and shows nothing"
    recorded.write_text("0.0.1")
    assert Changelog(fake).pending == [] and recorded.read_text() == "0.0.3", "off by default, the version is still recorded"
    fake.setConfig("desktop.whats_new", "true")
    recorded.write_text("0.0.1")
    changelog = Changelog(fake)
    assert versions(changelog.pending) == ["0.0.3", "0.0.2"]
    assert versions(changelog.releases) == ["0.0.3", "0.0.2", "0.0.1"]
    assert Changelog(fake).pending == [], "the next start has nothing new"


def test_the_launch_page_turns_whats_new_on(api, fake):
    form = api.screens.launch
    form.load()
    row = until(lambda: next((r for r in form.rows if r["key"] == "desktop.whats_new"), None))
    assert row["value"] is False and not row["advanced"], "off by default, shown without Advanced"


@pytest.fixture
def look(request, upgraded, tmp_path):
    """A look started on the upgraded core."""
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    api = Api(upgraded, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    shown = Look(api, request.param)
    yield shown
    shown.close()
    api.shutdown()


def test_an_update_opens_whats_new_once_and_about_opens_the_changelog(look):
    changelog = look.api.screens.changelog
    page = look.page("changelogPage")
    assert page.property("fresh") is True and versions(read(page, "releases")) == ["0.0.3", "0.0.2"], "the start opens what's new"
    look.press(Qt.Key.Key_Escape)
    until(lambda: changelog.pending == [], "B closes it, seen")

    about = look.settings("about")
    until(lambda: about.property("sectionId") == "about")
    content = read(about, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    row = next(r for r in rows if r.get("key") == "changelog")
    invoke(about, "activate", rows.index(row), row)
    page = until(lambda: (p := look.page("changelogPage")).property("fresh") is False and p, "About › Changelog opens it")
    assert versions(read(page, "releases")) == ["0.0.3", "0.0.2", "0.0.1"]
