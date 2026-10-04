import pytest
from test_render import page_as, render

from conftest import until

LOOKS = ["reprise", "switch2", "ps5"]


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


def value(item, name):
    v = item.property(name)
    return v.toVariant() if hasattr(v, "toVariant") else v


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


def changelog_page(root, theme):
    from PySide6.QtCore import QObject

    if theme == "reprise":
        shown = root.property("subOpen") is True and root.property("subSource") == "pages/ChangelogPage.qml"
        return shown and root.findChild(QObject, "changelogPage")
    top = root.property("topPage")
    return top is not None and top.objectName() == "changelogPage" and top


def open_about(root, theme):
    from PySide6.QtCore import Q_ARG, QMetaObject

    if theme == "reprise":
        root.goToTab(root.property("settingsTab"))
        page = page_as(root, "SettingsPage")
        QMetaObject.invokeMethod(page, "land", Q_ARG("QVariant", "about"))
    else:
        QMetaObject.invokeMethod(root, "push", Q_ARG("QVariant", "pages/SettingsPage.qml"), Q_ARG("QVariant", {"section": "about"}))
        page = until(lambda: (top := root.property("topPage")) is not None and top.objectName() != "changelogPage" and top)
    until(lambda: page.property("sectionId") == "about")
    return page


@pytest.mark.parametrize("theme", LOOKS)
def test_an_update_opens_whats_new_once_and_about_opens_the_changelog(upgraded, tmp_path, theme):
    from PySide6.QtCore import Q_ARG, QMetaObject, Qt
    from PySide6.QtTest import QTest

    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    api = Api(upgraded, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    api.theme.set(theme)
    api.theme.takeLanding()
    engine, window = render(api, activate=True)
    root = window.property("contentItem").childItems()[0].property("item")
    page = until(lambda: changelog_page(root, theme), "the start opens what's new")
    assert page.property("fresh") is True and versions(value(page, "releases")) == ["0.0.3", "0.0.2"]
    QTest.keyClick(window, Qt.Key.Key_Escape)
    until(lambda: not changelog_page(root, theme) and api.screens.changelog.pending == [], "B closes it, seen")

    about = open_about(root, theme)
    content = value(about, "content")
    rows = content["rows"] if isinstance(content, dict) else content
    row = next(r for r in rows if r.get("key") == "changelog")
    QMetaObject.invokeMethod(about, "activate", Q_ARG("QVariant", rows.index(row)), Q_ARG("QVariant", row))
    page = until(lambda: (p := changelog_page(root, theme)) and p.property("fresh") is False and p, "About › Changelog opens it")
    assert versions(value(page, "releases")) == ["0.0.3", "0.0.2", "0.0.1"]
    window.close()
    del engine
    api.shutdown()
