.pragma library

function roundRect(ctx, x, y, w, h, r) {
    r = Math.min(r, w / 2, h / 2);
    ctx.beginPath();
    ctx.roundedRect(x, y, w, h, r, r);
    ctx.closePath();
}

function circle(ctx, cx, cy, r) {
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.closePath();
}

// A cross of arm width `a` and half-length `l`, centred.
function cross(ctx, cx, cy, l, a) {
    var h = a / 2;
    ctx.beginPath();
    ctx.moveTo(cx - h, cy - l);
    ctx.lineTo(cx + h, cy - l);
    ctx.lineTo(cx + h, cy - h);
    ctx.lineTo(cx + l, cy - h);
    ctx.lineTo(cx + l, cy + h);
    ctx.lineTo(cx + h, cy + h);
    ctx.lineTo(cx + h, cy + l);
    ctx.lineTo(cx - h, cy + l);
    ctx.lineTo(cx - h, cy + h);
    ctx.lineTo(cx - l, cy + h);
    ctx.lineTo(cx - l, cy - h);
    ctx.lineTo(cx - h, cy - h);
    ctx.closePath();
}

function arm(ctx, cx, cy, l, a, dir) {
    var h = a / 2;
    var dx = dir === "left" ? -1 : dir === "right" ? 1 : 0;
    var dy = dir === "up" ? -1 : dir === "down" ? 1 : 0;
    var x = cx + dx * (h + l) / 2, y = cy + dy * (h + l) / 2;
    var w = dx !== 0 ? l - h : a, hh = dy !== 0 ? l - h : a;
    ctx.beginPath();
    ctx.rect(x - w / 2, y - hh / 2, w, hh);
    ctx.closePath();
}

function paddle(ctx, cx, y, w, h) {
    ctx.beginPath();
    ctx.moveTo(cx - w * 0.32, y);
    ctx.lineTo(cx + w * 0.32, y);
    ctx.bezierCurveTo(cx + w * 0.5, y + h * 0.25, cx + w * 0.5, y + h * 0.75, cx + w * 0.34, y + h);
    ctx.lineTo(cx - w * 0.34, y + h);
    ctx.bezierCurveTo(cx - w * 0.5, y + h * 0.75, cx - w * 0.5, y + h * 0.25, cx - w * 0.32, y);
    ctx.closePath();
}

// The marks printed on buttons, on a unit radius about the origin: stroked, `fill` ones filled.
var STAR = (function() {
    var d = "";
    for (var k = 0; k < 10; k++) {
        var r = k % 2 === 0 ? 1.05 : 0.45, a = -Math.PI / 2 + k * Math.PI / 5;
        d += (k === 0 ? "M" : "L") + (Math.cos(a) * r).toFixed(4) + " " + (Math.sin(a) * r).toFixed(4);
    }
    return d + "Z";
})();
var SYMBOLS = {
    cross: "M-1 -1L1 1M1 -1L-1 1",
    circle: "M1 0A1 1 0 1 1 -1 0A1 1 0 1 1 1 0",
    triangle: "M0 -1.0976L1.12 0.7392L-1.12 0.7392Z",
    square: "M-0.94 -0.94H0.94V0.94H-0.94Z",
    menu: "M-1 -0.75H1M-1 0H1M-1 0.75H1",
    create: "M-1 -0.75H0.2M-1 0H1M-1 0.75H-0.2",
    view: "M-0.8 -0.44H0.28V0.52H-0.8ZM-0.28 -0.76H0.8V0.2H-0.28Z",
    minus: "M-1 0H1",
    plus: "M-1 0H1M0 -1V1",
    home: "M-1 0L0 -1L1 0M-0.7 -0.25V0.9H0.7V-0.25",
    xbox: "M1.1 0A1.1 1.1 0 1 1 -1.1 0A1.1 1.1 0 1 1 1.1 0M-0.55 -0.55Q0 0 0.55 0.55M0.55 -0.55Q0 0 -0.55 0.55",
    share: "M0 0.5V-1M-0.55 -0.45L0 -1L0.55 -0.45M-0.9 0V1H0.9V0",
    capture: "M-1 -1H1V1H-1ZM0.4 0A0.4 0.4 0 1 1 -0.4 0A0.4 0.4 0 1 1 0.4 0",
    mic: "M-0.35 -0.65A0.35 0.35 0 0 1 0.35 -0.65V-0.05A0.35 0.35 0 0 1 -0.35 -0.05ZM0.6237 0.3178A0.7 0.7 0 0 1 -0.6237 0.3178M0 0.7V1.05",
    star: STAR,
    dot: { fill: "M0.35 0A0.35 0.35 0 1 1 -0.35 0A0.35 0.35 0 1 1 0.35 0" }
};

function symbol(ctx, name, cx, cy, r) {
    var sym = SYMBOLS[name];
    if (!sym)
        return;
    var s = r * 0.56;
    ctx.save();
    ctx.translate(cx, cy);
    ctx.scale(s, s);
    ctx.lineWidth /= s;
    ctx.beginPath();
    ctx.path = sym.fill || sym;
    if (sym.fill)
        ctx.fill();
    else
        ctx.stroke();
    ctx.restore();
}
