.pragma library

// The near-global hints go right, so they never move as the labels around them change.
var LEFT = ["A", "X", "Y", "RS", "LS", "dpad", "Start", "Select", "Start+Select", "B"];
var RIGHT = ["LT RT", "LB RB"];

function rank(order, glyph) {
    var i = order.indexOf(glyph);
    if (i >= 0)
        return i;
    var slot = glyph.split(/[ +]/)[0];
    i = order.indexOf(slot);
    return i >= 0 ? i + 0.5 : order.length;
}

function sorted(hints, order) {
    var out = hints.slice();
    for (var i = 0; i < out.length; i++)
        out[i]._at = i;
    out.sort(function(a, b) {
        var d = rank(order, a.glyph) - rank(order, b.glyph);
        return d !== 0 ? d : a._at - b._at;
    });
    return out;
}

function arrange(hints) {
    var left = [], right = [];
    for (var i = 0; i < (hints || []).length; i++) {
        var h = hints[i];
        if (!h || !h.glyph)
            continue;
        (RIGHT.indexOf(h.glyph) >= 0 ? right : left).push(h);
    }
    return { left: sorted(left, LEFT), right: sorted(right, RIGHT) };
}
