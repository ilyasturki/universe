def values(layout, row):
    return "".join(k["value"] for k in layout["rows"][row])


def shifts(layout, row):
    return "".join(k["shift"] for k in layout["rows"][row])


def test_the_rows_are_the_physical_keyboards():
    from universe_ui import keyboard

    fr = keyboard.rows("fr")
    assert fr["name"] == "fr"
    assert values(fr, "AD") == "azertyuiop$" and shifts(fr, "AD") == "AZERTYUIOP£", "the dead ^ is left out"
    assert values(fr, "AC") == "qsdfghjklmù*" and shifts(fr, "AC") == "QSDFGHJKLM%µ"
    assert values(fr, "AB") == "wxcvbn,;:!" and shifts(fr, "AB") == "WXCVBN?./§"
    assert values(fr, "AE") == "1234567890)=" and shifts(fr, "AE") == "&é\"'(-è_çà°+", "the digits lead the number row, the layout's own level on shift"
    us = keyboard.rows("us")
    assert values(us, "AD") == "qwertyuiop[]" and values(us, "AE") == "1234567890-=" and shifts(us, "AE") == "!@#$%^&*()_+"
    bepo = keyboard.rows("fr", "bepo")
    assert bepo["name"] == "fr:bepo" and values(bepo, "AD") == "bépoèvdljzw"


def test_an_unknown_layout_shows_qwerty():
    from universe_ui import keyboard

    rows = keyboard.rows("nope")
    assert rows["name"] == "nope" and values(rows, "AD") == "qwertyuiop[]" and shifts(rows, "AB") == "ZXCVBNM<>?"


def test_the_keys_carry_the_sessions_layout(fake, tmp_path, monkeypatch):
    from universe_ui.api import Api
    from universe_ui.screens.power import FAKE

    monkeypatch.setenv("XKB_DEFAULT_LAYOUT", "fr,us")
    monkeypatch.setenv("XKB_DEFAULT_VARIANT", ",")
    api = Api(fake, memory_path=str(tmp_path / "memory.json"), power_root=FAKE)
    try:
        layout = api.keys.layout
        assert layout["name"] == "fr" and layout["rows"]["AD"][0] == {"value": "a", "shift": "A"}
    finally:
        api.shutdown()
