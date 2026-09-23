import logging

log = logging.getLogger("universe.keyboard")

# The physical rows, by xkb key name: the number row and the three letter rows, ISO width.
ROWS = {"AE": 12, "AD": 12, "AC": 12, "AB": 11}

QWERTY = {
    "AE": "1! 2@ 3# 4$ 5% 6^ 7& 8* 9( 0) -_ =+",
    "AD": "qQ wW eE rR tT yY uU iI oO pP [{ ]}",
    "AC": "aA sS dD fF gG hH jJ kK lL ;: '\"",
    "AB": "zZ xX cC vV bB nN mM ,< .> /?",
}


def _pairs(spec):
    return [{"value": k[0], "shift": k[1]} for k in spec.split(" ")]


# `{name, rows: {AE, AD, AC, AB: [{value, shift}]}}`, dead keys left out, the digits leading the number row; `us` when the layout is missing.
def rows(layout, variant=""):
    name = f"{layout}:{variant}" if variant else layout
    try:
        keys = _compiled(layout, variant)
    except Exception as e:  # noqa: BLE001  cffi raises its own kinds; any of them means the fallback
        log.warning("keyboard layout %s: %s; showing qwerty", name, e)
        keys = {row: _pairs(spec) for row, spec in QWERTY.items()}
    return {"name": name, "rows": keys}


def _compiled(layout, variant):
    from xkbcommon import xkb

    keymap = xkb.Context().keymap_new_from_names(layout=layout, variant=variant or None)
    out = {}
    for row, count in ROWS.items():
        keys = []
        for n in range(1, count + 1):
            code = keymap.key_by_name(f"{row}{n:02}")
            levels = [_char(xkb, keymap, code, level) for level in range(min(2, keymap.num_levels_for_key(code, 0)))]
            value = levels[0] if levels else None
            shift = levels[1] if len(levels) > 1 else None
            if row == "AE" and shift and shift.isdigit() and not (value and value.isdigit()):
                value, shift = shift, value
            if not value:
                continue
            keys.append({"value": value, "shift": shift or value.upper()})
        out[row] = keys
    if not out["AD"] or not out["AC"] or not out["AB"]:
        raise ValueError("no letter rows")
    return out


def _char(xkb, keymap, code, level):
    syms = keymap.key_get_syms_by_level(code, 0, level)
    text = xkb.keysym_to_string(syms[0]) if syms else None
    return text if text and text.strip() and text.isprintable() else None
