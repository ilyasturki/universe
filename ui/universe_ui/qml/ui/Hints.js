.pragma library

// The near-global hint goes right, so it never moves as the labels around it change.
var RIGHT = ["LB RB"];

function arrange(hints) {
    var out = { left: [], right: [] };
    for (var i = 0; i < (hints || []).length; i++) {
        var h = hints[i];
        if (h && h.glyph)
            (RIGHT.indexOf(h.glyph) >= 0 ? out.right : out.left).push(h);
    }
    return out;
}
