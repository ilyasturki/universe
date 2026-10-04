import _scummvm
from _controls import Context, ini_section
from controls_fixtures import EDGE, XBOX


def test_joystick_num_follows_the_first_pad_where_unset_reads_zero_and_minus_one_is_left_off(tmp_path):
    ini = tmp_path / "scummvm" / "scummvm.ini"
    assert _scummvm.plan(Context([EDGE])) == {}
    assert ini_section(_scummvm.plan(Context([XBOX, EDGE]))[ini], "scummvm") == {"joystick_num": "1"}
    ini.parent.mkdir()
    ini.write_text("[scummvm]\njoystick_num=3\n")
    assert ini_section(_scummvm.plan(Context([EDGE]))[ini], "scummvm")["joystick_num"] == "0"
    ini.write_text("[scummvm]\njoystick_num=-1\n")
    assert _scummvm.plan(Context([XBOX])) == {}


def test_the_legacy_scummvmrc_wins(tmp_path):
    rc = tmp_path / ".scummvmrc"
    rc.write_text("[scummvm]\n")
    assert list(_scummvm.plan(Context([XBOX]))) == [rc]
