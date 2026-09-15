.pragma library

// The near-global hints go right, so they never move as the labels around them change.
var RIGHT = ["LT RT", "LB RB"];

function arrange(hints) {
    var out = { left: [], right: [] };
    for (var i = 0; i < (hints || []).length; i++) {
        var h = hints[i];
        if (h && h.glyph)
            (RIGHT.indexOf(h.glyph) >= 0 ? out.right : out.left).push(h);
    }
    return out;
}
