.pragma library

// Canvas strokes shared by the button glyphs and the pad art: outlines and the symbols printed on
// a pad's buttons, all on a unit radius so one call serves a 14 px glyph and a 60 px button.

function roundRect(ctx, x, y, w, h, r) {
    r = Math.min(r, w / 2, h / 2);
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.arcTo(x + w, y, x + w, y + r, r);
    ctx.lineTo(x + w, y + h - r);
    ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
    ctx.lineTo(x + r, y + h);
    ctx.arcTo(x, y + h, x, y + h - r, r);
    ctx.lineTo(x, y + r);
    ctx.arcTo(x, y, x + r, y, r);
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

// One arm of that cross, from the centre out.
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

// A back paddle: a lever, wider at the tip, hanging from y.
function paddle(ctx, cx, y, w, h) {
    ctx.beginPath();
    ctx.moveTo(cx - w * 0.32, y);
    ctx.lineTo(cx + w * 0.32, y);
    ctx.bezierCurveTo(cx + w * 0.5, y + h * 0.25, cx + w * 0.5, y + h * 0.75, cx + w * 0.34, y + h);
    ctx.lineTo(cx - w * 0.34, y + h);
    ctx.bezierCurveTo(cx - w * 0.5, y + h * 0.75, cx - w * 0.5, y + h * 0.25, cx - w * 0.32, y);
    ctx.closePath();
}

// The symbol printed on a button, stroked (or filled) inside radius r about (cx, cy).
function symbol(ctx, name, cx, cy, r) {
    var s = r * 0.56;
    ctx.beginPath();
    if (name === "cross") {
        ctx.moveTo(cx - s, cy - s);
        ctx.lineTo(cx + s, cy + s);
        ctx.moveTo(cx + s, cy - s);
        ctx.lineTo(cx - s, cy + s);
        ctx.stroke();
    } else if (name === "circle") {
        ctx.arc(cx, cy, s, 0, Math.PI * 2);
        ctx.stroke();
    } else if (name === "triangle") {
        var t = s * 1.12;
        ctx.moveTo(cx, cy - t * 0.98);
        ctx.lineTo(cx + t, cy + t * 0.66);
        ctx.lineTo(cx - t, cy + t * 0.66);
        ctx.closePath();
        ctx.stroke();
    } else if (name === "square") {
        var q = s * 0.94;
        ctx.rect(cx - q, cy - q, q * 2, q * 2);
        ctx.stroke();
    } else if (name === "menu") {
        for (var i = -1; i <= 1; i++) {
            ctx.moveTo(cx - s, cy + i * s * 0.75);
            ctx.lineTo(cx + s, cy + i * s * 0.75);
        }
        ctx.stroke();
    } else if (name === "create") {
        ctx.moveTo(cx - s, cy - s * 0.75);
        ctx.lineTo(cx + s * 0.2, cy - s * 0.75);
        ctx.moveTo(cx - s, cy);
        ctx.lineTo(cx + s, cy);
        ctx.moveTo(cx - s, cy + s * 0.75);
        ctx.lineTo(cx - s * 0.2, cy + s * 0.75);
        ctx.stroke();
    } else if (name === "view") {
        var v = s * 0.8;
        ctx.rect(cx - v, cy - v * 0.55, v * 1.35, v * 1.2);
        ctx.stroke();
        ctx.beginPath();
        ctx.rect(cx - v * 0.35, cy - v * 0.95, v * 1.35, v * 1.2);
        ctx.stroke();
    } else if (name === "minus") {
        ctx.moveTo(cx - s, cy);
        ctx.lineTo(cx + s, cy);
        ctx.stroke();
    } else if (name === "plus") {
        ctx.moveTo(cx - s, cy);
        ctx.lineTo(cx + s, cy);
        ctx.moveTo(cx, cy - s);
        ctx.lineTo(cx, cy + s);
        ctx.stroke();
    } else if (name === "home") {
        ctx.moveTo(cx - s, cy);
        ctx.lineTo(cx, cy - s);
        ctx.lineTo(cx + s, cy);
        ctx.moveTo(cx - s * 0.7, cy - s * 0.25);
        ctx.lineTo(cx - s * 0.7, cy + s * 0.9);
        ctx.lineTo(cx + s * 0.7, cy + s * 0.9);
        ctx.lineTo(cx + s * 0.7, cy - s * 0.25);
        ctx.stroke();
    } else if (name === "xbox") {
        ctx.arc(cx, cy, s * 1.1, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(cx - s * 0.55, cy - s * 0.55);
        ctx.quadraticCurveTo(cx, cy, cx + s * 0.55, cy + s * 0.55);
        ctx.moveTo(cx + s * 0.55, cy - s * 0.55);
        ctx.quadraticCurveTo(cx, cy, cx - s * 0.55, cy + s * 0.55);
        ctx.stroke();
    } else if (name === "share") {
        ctx.moveTo(cx, cy + s * 0.5);
        ctx.lineTo(cx, cy - s);
        ctx.moveTo(cx - s * 0.55, cy - s * 0.45);
        ctx.lineTo(cx, cy - s);
        ctx.lineTo(cx + s * 0.55, cy - s * 0.45);
        ctx.moveTo(cx - s * 0.9, cy);
        ctx.lineTo(cx - s * 0.9, cy + s);
        ctx.lineTo(cx + s * 0.9, cy + s);
        ctx.lineTo(cx + s * 0.9, cy);
        ctx.stroke();
    } else if (name === "capture") {
        ctx.rect(cx - s, cy - s, s * 2, s * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(cx, cy, s * 0.4, 0, Math.PI * 2);
        ctx.stroke();
    } else if (name === "mic") {
        roundRect(ctx, cx - s * 0.35, cy - s, s * 0.7, s * 1.3, s * 0.35);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(cx, cy, s * 0.7, Math.PI * 0.15, Math.PI * 0.85);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(cx, cy + s * 0.7);
        ctx.lineTo(cx, cy + s * 1.05);
        ctx.stroke();
    } else if (name === "star") {
        for (var k = 0; k < 10; k++) {
            var rr = k % 2 === 0 ? s * 1.05 : s * 0.45;
            var ang = -Math.PI / 2 + k * Math.PI / 5;
            var px = cx + Math.cos(ang) * rr, py = cy + Math.sin(ang) * rr;
            if (k === 0)
                ctx.moveTo(px, py);
            else
                ctx.lineTo(px, py);
        }
        ctx.closePath();
        ctx.stroke();
    } else if (name === "dot") {
        ctx.arc(cx, cy, s * 0.35, 0, Math.PI * 2);
        ctx.fill();
    }
}
