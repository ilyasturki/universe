import _mapping
from controls_fixtures import EDGE, SWITCH_PRO, XBOX, twin


def fields(line):
    return dict(f.split(":", 1) for f in line.rstrip(",").split(",")[2:])


def test_universes_buttons_leave_the_mapping_and_the_rest_stays_with_guide_when_passed_through():
    got = fields(_mapping.stripped(EDGE.mapping))
    assert not {"guide", "misc1", "paddle1", "paddle2", "paddle3", "paddle4"} & set(got)
    assert got["a"] == "b0" and got["touchpad"] == "b11" and got["crc"] == "27e0"
    assert _mapping.stripped(EDGE.mapping).startswith("0500e0274c050000f20d000000006800,*,")
    assert fields(_mapping.stripped(EDGE.mapping, guide=True))["guide"] == "b5"


def test_a_line_without_a_platform_gets_one():
    assert _mapping.stripped("00ff,Pad,a:b0").endswith(",platform:Linux,")


def test_one_line_per_model_after_the_emulators_databases(tmp_path):
    db = tmp_path / "db.txt"
    db.write_text("# a comment\n0500e0274c050000f20d000000006800,Old,a:b1,guide:b5,platform:Linux,\n")
    lines = _mapping.text([EDGE, XBOX, twin(EDGE), SWITCH_PRO], databases=[db, tmp_path / "missing.txt"]).splitlines()
    assert lines[0] == "# a comment" and lines[1].endswith(",Old,a:b1,guide:b5,platform:Linux,")
    assert [line.split(",", 1)[0] for line in lines[2:]] == [p.guid.hex() for p in (EDGE, XBOX, SWITCH_PRO)]
    assert "guide" not in lines[-1] and "misc1" not in lines[-1]
