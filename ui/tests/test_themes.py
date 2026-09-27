from universe_ui import host
from universe_ui.api import Memory
from universe_ui.themes import THEMES, ThemeSelector


def test_theme_ids_are_unique_and_have_entries():
    ids = [t["id"] for t in THEMES]
    assert len(ids) == len(set(ids))
    for theme in THEMES:
        assert (host.QML_DIR / theme["entry"]).is_file(), theme["entry"]


def test_selector_defaults_and_persists(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    selector = ThemeSelector(memory)
    assert selector.current == "reprise" and selector.entry == "theme.qml"
    assert selector.set("switch2") is True
    assert selector.current == "switch2" and selector.name == "Switch 2"
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
