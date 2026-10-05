import shutil
from pathlib import Path

import pytest
from looks import LOOKS, Look, call, invoke, read, theme_rows
from PySide6.QtCore import QUrl
from uitest import until

from universe_ui import host
from universe_ui.api import Memory
from universe_ui.themes import THEMES, ThemeSelector

SAMPLE = Path(__file__).resolve().parents[2] / "examples" / "theme"


def test_theme_ids_are_unique_and_have_entries():
    ids = [t["id"] for t in THEMES]
    assert len(ids) == len(set(ids))
    for theme in THEMES:
        assert (host.QML_DIR / theme["entry"]).is_file(), theme["entry"]
        assert (host.QML_DIR / theme["osd"]).is_file(), f"{theme['id']} draws the volume level over the game"
        assert theme["unlocked"], f"{theme['id']} words the overlay's unlock card"
        assert theme["accent"] and theme["ground"], f"{theme['id']}'s swatch in Themes"


def test_the_unlock_card_takes_the_looks_words(app, tmp_path):
    selector = ThemeSelector(Memory(str(tmp_path / "memory.json")))
    for theme in THEMES:
        selector.set(theme["id"])
        assert selector.unlocked == theme["unlocked"]


def test_selector_defaults_and_persists(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    selector = ThemeSelector(memory)
    assert selector.current == "reprise" and selector.entry == "theme.qml"
    assert selector.set("switch2") is True
    assert selector.current == "switch2"
    assert selector.entry == "switch2/theme.qml"
    assert memory.get("theme") == "switch2"
    assert selector.set("no-such-theme") is False and selector.current == "switch2"
    assert ThemeSelector(memory).current == "switch2"
    assert ThemeSelector(memory, "reprise").current == "reprise"
    memory.set("theme", "gone")
    assert ThemeSelector(memory).current == "reprise"
    assert selector.fontPath == ""
    selector.fontPath = "/fonts/udsg.ttf"
    assert memory.get("switch2Font") == "/fonts/udsg.ttf" and selector.fontPath == "/fonts/udsg.ttf"
    selector.fontPath = ""
    assert memory.has("switch2Font") is False


def test_the_startup_animation_is_on_until_turned_off_for_every_look(app, fake, tmp_path):
    from universe_ui.api import Api

    path = str(tmp_path / "memory.json")
    selector = ThemeSelector(Memory(path), "switch2")
    assert selector.bootIntro is True
    selector.bootIntro = False
    selector.set("ps5")
    assert selector.bootIntro is False and ThemeSelector(Memory(path)).bootIntro is False
    api = Api(fake, memory_path=path, boot=True)
    assert api.boot.running is False, "the switch wins over a start that would play it"
    api.shutdown()


def test_each_look_keeps_its_own_font(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    selector = ThemeSelector(memory, "switch2")
    selector.fontPath = "/fonts/udsg.ttf"
    fonts = []
    selector.fontChanged.connect(lambda: fonts.append(selector.fontPath))
    assert selector.set("ps5") is True
    assert selector.entry == "ps5/theme.qml" and selector.frame is False
    assert fonts == [""] and selector.fontPath == ""
    selector.fontPath = "/fonts/sst.ttf"
    assert memory.get("ps5Font") == "/fonts/sst.ttf" and memory.get("switch2Font") == "/fonts/udsg.ttf"
    selector.set("switch2")
    assert selector.fontPath == "/fonts/udsg.ttf"


@pytest.mark.parametrize("name", LOOKS)
def test_a_look_s_sounds_load_before_the_api_has_a_theme(app, name):
    from PySide6.QtCore import QObject, QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    sound = (host.QML_DIR / next(t["entry"] for t in THEMES if t["id"] == name)).parent / "sound"
    engine = QQmlEngine()
    bare = QObject()
    engine.rootContext().setContextProperty("api", bare)
    component = QQmlComponent(engine)
    component.setData(f'import QtQuick\nimport "{sound.as_uri()}"\nQtObject {{ property var out: Sound.overrides }}\n'.encode(), QUrl("file:///sound.qml"))
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    assert read(obj, "out") == {}


def test_a_pool_started_on_a_folder_goes_back_to_its_bundled_sounds(app):
    from PySide6.QtCore import QUrl
    from PySide6.QtQml import QQmlComponent, QQmlEngine

    sounds = host.QML_DIR / "assets" / "sounds"
    engine = QQmlEngine()
    component = QQmlComponent(engine)
    qml = (
        f'import QtQuick\nimport "{(host.QML_DIR / "sound").as_uri()}"\n'
        f'SoundPool {{ dir: "{sounds.as_uri()}/"; poolSizes: ({{tick: 1}}); overrides: ({{tick: "{(sounds / "edge.wav").as_uri()}"}}) }}\n'
    )
    component.setData(qml.encode(), QUrl("file:///pool.qml"))
    pool = component.create()
    assert pool is not None, [e.toString() for e in component.errors()]
    pool.preload()

    def tick():
        return pool.property("voices").toVariant()["tick"][0]["fx"].property("source").toString()

    assert tick().endswith("/edge.wav")
    pool.setProperty("overrides", {})
    assert tick().endswith("/tick.wav"), "an emptied folder hands the bundled sound back"


def test_a_sound_folder_replaces_the_wavs_it_holds(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    selector = ThemeSelector(memory, "switch2")
    folder = tmp_path / "sounds"
    folder.mkdir()
    (folder / "ok.wav").write_bytes(b"")
    (folder / "Tick.WAV").write_bytes(b"")
    (folder / "notes.txt").write_text("")
    assert selector.soundFiles == {}
    selector.soundsPath = str(folder)
    assert memory.get("switch2Sounds") == str(folder)
    assert selector.soundFiles == {"tick": f"file://{folder}/Tick.WAV", "ok": f"file://{folder}/ok.wav"}
    selector.set("ps5")
    assert selector.soundsPath == "" and selector.soundFiles == {}
    selector.set("switch2")
    selector.soundsPath = str(tmp_path / "gone")
    assert selector.soundFiles == {}


DUSK = {"id": "dusk", "name": "Dusk", "entry": "/themes/dusk/theme.qml", "screenshot": "/themes/dusk/shot.png", "description": "", "incompatible": ""}
OLD = {**DUSK, "id": "old", "name": "Old", "incompatible": "written for extension api 9: this Universe reads api 2"}


def test_an_installed_theme_lists_beside_the_looks_and_one_for_another_universe_cannot_be_picked(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    memory.set("theme", "old")
    selector = ThemeSelector(memory, installed=lambda: [DUSK, OLD, {**DUSK, "id": "ps5"}])
    assert [t["id"] for t in selector.themes] == ["reprise", "switch2", "ps5", "dusk", "old"], "a built-in look keeps its id"
    assert next(t for t in selector.themes if t["id"] == "old")["unavailable"]
    assert selector.current == "reprise" and selector.takeNotice(), "remembered but unusable: the default look, with a notice"
    assert selector.takeNotice() == "", "said once"
    assert selector.set("old") is False and selector.current == "reprise"
    assert selector.set("dusk") is True
    assert selector.entry == QUrl.fromLocalFile(DUSK["entry"]).toString()
    assert (selector.overlay, selector.osd, selector.frame) == ("", "ui/VolumePill.qml", False)


def test_a_removed_theme_hands_over_to_the_default_look(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    shelf = [DUSK]
    selector = ThemeSelector(memory, installed=lambda: list(shelf))
    selector.set("dusk")
    listed = []
    selector.listChanged.connect(lambda: listed.append(True))
    shelf.clear()
    selector.rescan()
    assert listed and "dusk" not in [t["id"] for t in selector.themes]
    assert (selector.current, memory.get("theme")) == ("reprise", "reprise")
    assert selector.takeNotice()


def notice(look):
    """What the shared notices show now, as [text, error], or None."""
    from PySide6.QtQml import QQmlComponent

    component = QQmlComponent(look.engine)
    now = "Notices.current ? [Notices.current.text, Notices.current.error] : null"
    component.setData(
        f'import QtQuick\nimport "{(host.QML_DIR / "core").as_uri()}"\nQtObject {{ function now() {{ return {now}; }} }}\n'.encode(), QUrl("file:///probe.qml")
    )
    obj = component.create()
    assert obj is not None, [e.toString() for e in component.errors()]
    return call(obj, "now")


def test_a_theme_for_another_universe_is_listed_but_cannot_be_picked(look, api, fake):
    from PySide6.QtCore import QCoreApplication

    old = Path(fake.core.data_home()) / "extensions" / "theme" / "old"
    shutil.copytree(SAMPLE, old)
    (old / "theme.toml").write_text((SAMPLE / "theme.toml").read_text().replace('id = "sample"', 'id = "old"').replace("api = 2", "api = 9"))
    page = look.settings("themes")
    until(lambda: any(t["id"] == "old" for t in api.theme.themes), "the Themes section reads the installed themes")
    if look.stacked:
        at, row = until(lambda: next(((i, r) for i, r in enumerate(theme_rows(page)) if r.get("theme") == "old"), None), "listed")
        assert row["dim"] is True
        invoke(page, "activate", at, row)
        QCoreApplication.processEvents()
    else:
        at = [t["id"] for t in api.theme.themes].index("old")
        assert call(page, "themeItems")[at]["action"] == "", "its pick in the Theme menu picks nothing"
    assert api.theme.set("old") is False
    assert api.theme.current == look.name


@pytest.mark.slow
@pytest.mark.qt_log_ignore(r"broken/theme\.qml", extend=True)
def test_the_sample_theme_installs_from_a_folder_switches_falls_back_when_broken_and_goes(api, fake, tmp_path):
    look = Look(api, "reprise")
    try:
        fake.core.extension_install(str(SAMPLE), True)
        api.theme.rescan()
        assert next(t for t in api.theme.themes if t["id"] == "sample")["unavailable"] == ""
        look.switch("sample")
        until(lambda: look.root.objectName() == "sampleTheme", "the sample's own tree is the look")

        broken = tmp_path / "broken"
        shutil.copytree(SAMPLE, broken)
        (broken / "theme.toml").write_text((SAMPLE / "theme.toml").read_text().replace('id = "sample"', 'id = "broken"'))
        (broken / "theme.qml").write_text("import QtQuick\nItem {\n")
        fake.core.extension_install(str(broken), True)
        api.theme.rescan()
        look.switch("broken")
        until(lambda: api.theme.current == "reprise", "a theme that fails to load falls back")
        assert api.memory.get("theme") == "reprise", "and the next start does not try it again"
        shown = until(lambda: notice(look), "with a notice")
        assert shown[1] is True, shown

        look.switch("sample")
        until(lambda: look.root.objectName() == "sampleTheme")
        fake.core.extension_remove("sample")
        api.theme.rescan()
        until(lambda: api.theme.current == "reprise", "removed, the default look takes over")
        assert "sample" not in [t["id"] for t in api.theme.themes]
    finally:
        look.close()
