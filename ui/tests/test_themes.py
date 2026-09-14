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


def test_legacy_variant_ids_still_select_switch2(app, tmp_path):
    memory = Memory(str(tmp_path / "memory.json"))
    memory.set("theme", "switch2-black")
    assert ThemeSelector(memory).current == "switch2"
    selector = ThemeSelector(memory)
    assert selector.set("switch2-white") is True and selector.current == "switch2"
    assert memory.get("theme") == "switch2"


def test_theme_flag_is_parsed():
    assert host.parse_args(["--theme", "switch2"]).theme == "switch2"
