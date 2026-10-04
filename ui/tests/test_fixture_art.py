import pytest
from PySide6.QtGui import QImage

from universe_ui.fixtures.art import SLOTS, _paint


@pytest.mark.parametrize("title", ["LEGO Batman: The Videogame", "LEGO Batman: The Videogame of the Very Long Title"])
def test_a_long_logo_title_shrinks_to_fit_whole_in_its_canvas(app, tmp_path, title):
    path = str(tmp_path / "logo.png")
    w, h = SLOTS["logo"]
    _paint(path, (w, h), "lego-batman", title, "logo")
    image = QImage(path)
    inked = next(y for y in range(h) if any(image.pixelColor(x, y).alpha() > 0 for x in range(0, w, 3)))
    # Text that overflows the drawing rect is cut at its top edge, the margin.
    assert inked > w // 12, "the first line is whole"
