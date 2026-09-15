.pragma library

// Each family's pad seen from the front, on a 1000 × 700 sheet.

var VIEW = [1000, 700];


var SONY_BODY = "M 500 100 C 600 100 705 110 780 134 C 845 155 890 200 912 262 C 930 312 938 370 936 430 "
    + "C 934 500 940 575 912 622 C 890 660 818 668 776 634 C 745 608 728 552 690 505 C 650 458 565 470 500 474 "
    + "C 435 470 350 458 310 505 C 272 552 255 608 224 634 C 182 668 110 660 88 622 C 60 575 66 500 64 430 "
    + "C 62 370 70 312 88 262 C 110 200 155 155 220 134 C 295 110 400 100 500 100 Z";

function sonyButtons(kind) {
    var b = [
        { slot: "lt", kind: "trigger", x: 250, y: 84, w: 96, h: 50 },
        { slot: "rt", kind: "trigger", x: 750, y: 84, w: 96, h: 50 },
        { slot: "lb", kind: "bumper", x: 250, y: 128, w: 144, h: 36 },
        { slot: "rb", kind: "bumper", x: 750, y: 128, w: 144, h: 36 },
        { slot: "select", kind: "small", x: 336, y: 200, w: 15, h: 44, angle: -16 },
        { slot: "start", kind: "small", x: 664, y: 200, w: 15, h: 44, angle: 16 },
        { slot: "dpad_up", kind: "arm", cx: 200, cy: 305, l: 64, a: 40, dir: "up", split: true },
        { slot: "dpad_down", kind: "arm", cx: 200, cy: 305, l: 64, a: 40, dir: "down", split: true },
        { slot: "dpad_left", kind: "arm", cx: 200, cy: 305, l: 64, a: 40, dir: "left", split: true },
        { slot: "dpad_right", kind: "arm", cx: 200, cy: 305, l: 64, a: 40, dir: "right", split: true },
        { slot: "north", kind: "face", x: 800, y: 245, r: 27 },
        { slot: "south", kind: "face", x: 800, y: 365, r: 27 },
        { slot: "west", kind: "face", x: 740, y: 305, r: 27 },
        { slot: "east", kind: "face", x: 860, y: 305, r: 27 },
        { slot: "ls", kind: "stick", x: 352, y: 440, r: 60, axes: ["lx", "ly"] },
        { slot: "rs", kind: "stick", x: 648, y: 440, r: 60, axes: ["rx", "ry"] },
        { slot: "guide", kind: "face", x: 500, y: 456, r: 22, plain: true }
    ];
    if (kind === "dualsense" || kind === "dualsense-edge")
        b.push({ slot: "mute", kind: "small", x: 500, y: 516, w: 46, h: 15 });
    if (kind === "dualsense-edge") {
        b.push({ slot: "fn_left", kind: "tab", x: 402, y: 536, w: 44, h: 20 });
        b.push({ slot: "fn_right", kind: "tab", x: 598, y: 536, w: 44, h: 20 });
        b.push({ slot: "paddle_left", kind: "paddle", x: 292, y: 552, w: 54, h: 96, ghost: true });
        b.push({ slot: "paddle_right", kind: "paddle", x: 708, y: 552, w: 54, h: 96, ghost: true });
    }
    if (kind === "8bitdo-pro-3") {
        b.push({ slot: "star", kind: "small", x: 500, y: 524, w: 30, h: 30, round: true });
        b.push({ slot: "paddle_l4", kind: "tab", x: 160, y: 128, w: 40, h: 30 });
        b.push({ slot: "paddle_r4", kind: "tab", x: 840, y: 128, w: 40, h: 30 });
        b.push({ slot: "paddle_pl", kind: "paddle", x: 292, y: 552, w: 54, h: 96, ghost: true });
        b.push({ slot: "paddle_pr", kind: "paddle", x: 708, y: 552, w: 54, h: 96, ghost: true });
    }
    return b;
}

function sonyDetails(kind) {
    var d = [ { kind: "dish", x: 200, y: 305, r: 74 } ];
    if (kind !== "8bitdo-pro-3") {
        d.push({ kind: "panel", x: 500, y: 200, w: 290, h: 150, r: 30 });
        d.push({ kind: "line", path: "M 360 178 C 360 138 378 130 398 130 M 640 178 C 640 138 622 130 602 130", alpha: 0.35, width: 3 });
        d.push({ kind: "line", path: "M 355 256 C 400 268 600 268 645 256", alpha: 0.12, width: 1.5 });
    } else {
        d.push({ kind: "panel", x: 500, y: 300, w: 120, h: 60, r: 24, alpha: 0.03 });
    }
    return d;
}

function sony(kind) {
    return { view: VIEW, body: SONY_BODY, details: sonyDetails(kind), buttons: sonyButtons(kind) };
}


var XBOX_BODY = "M 500 110 C 600 110 700 118 768 140 C 848 166 900 236 922 326 C 940 410 934 530 896 606 "
    + "C 866 660 792 668 752 626 C 716 588 694 540 644 502 C 594 466 546 484 500 486 "
    + "C 454 484 406 466 356 502 C 306 540 284 588 248 626 C 208 668 134 660 104 606 "
    + "C 66 530 60 410 78 326 C 100 236 152 166 232 140 C 300 118 400 110 500 110 Z";

function xboxButtons(kind) {
    var b = [
        { slot: "lt", kind: "trigger", x: 255, y: 90, w: 96, h: 52 },
        { slot: "rt", kind: "trigger", x: 745, y: 90, w: 96, h: 52 },
        { slot: "lb", kind: "bumper", x: 255, y: 134, w: 150, h: 36 },
        { slot: "rb", kind: "bumper", x: 745, y: 134, w: 150, h: 36 },
        { slot: "ls", kind: "stick", x: 250, y: 265, r: 62, axes: ["lx", "ly"] },
        { slot: "rs", kind: "stick", x: 630, y: 415, r: 62, axes: ["rx", "ry"] },
        { slot: "north", kind: "face", x: 750, y: 207, r: 27 },
        { slot: "south", kind: "face", x: 750, y: 323, r: 27 },
        { slot: "west", kind: "face", x: 692, y: 265, r: 27 },
        { slot: "east", kind: "face", x: 808, y: 265, r: 27 },
        { slot: "guide", kind: "face", x: 500, y: 172, r: 27, plain: true },
        { slot: "select", kind: "small", x: 425, y: 258, w: 30, h: 30, round: true },
        { slot: "start", kind: "small", x: 575, y: 258, w: 30, h: 30, round: true }
    ];
    var disc = kind === "xbox-elite";
    ["up", "down", "left", "right"].forEach(function(dir) {
        b.push({ slot: "dpad_" + dir, kind: "arm", cx: 370, cy: 415, l: disc ? 52 : 58, a: disc ? 30 : 36, dir: dir, split: false });
    });
    if (kind === "xbox")
        b.push({ slot: "share", kind: "small", x: 500, y: 300, w: 30, h: 26 });
    if (kind === "xbox-elite") {
        b.push({ slot: "paddle_p3", kind: "paddle", x: 222, y: 560, w: 52, h: 96, ghost: true });
        b.push({ slot: "paddle_p4", kind: "paddle", x: 292, y: 590, w: 46, h: 70, ghost: true });
        b.push({ slot: "paddle_p1", kind: "paddle", x: 778, y: 560, w: 52, h: 96, ghost: true });
        b.push({ slot: "paddle_p2", kind: "paddle", x: 708, y: 590, w: 46, h: 70, ghost: true });
    }
    if (kind === "switch-pro")
        b.push({ slot: "capture", kind: "small", x: 425, y: 330, w: 28, h: 28 });
    return b;
}

function xboxDetails(kind) {
    var d = [];
    if (kind === "xbox-elite") {
        d.push({ kind: "dish", x: 370, y: 415, r: 66, alpha: 0.07 });
        d.push({ kind: "cross", x: 370, y: 415, l: 52, a: 30 });
        d.push({ kind: "dot", x: 500, y: 232, r: 9 });
    } else {
        d.push({ kind: "dish", x: 370, y: 415, r: 70, alpha: 0.035 });
        d.push({ kind: "cross", x: 370, y: 415, l: 58, a: 36 });
    }
    return d;
}

function xbox(kind) {
    return { view: VIEW, body: XBOX_BODY, details: xboxDetails(kind), buttons: xboxButtons(kind) };
}

var SONY_KINDS = { "dualsense-edge": true, "dualsense": true, "dualshock4": true, "8bitdo-pro-3": true };

function of(family) {
    if (SONY_KINDS[family])
        return sony(family);
    return xbox(family === "xbox-elite" || family === "xbox" || family === "switch-pro" ? family : "generic");
}

function box(spec) {
    if (spec.kind === "face" || spec.kind === "stick")
        return { x: spec.x - spec.r, y: spec.y - spec.r, w: spec.r * 2, h: spec.r * 2 };
    if (spec.kind === "arm")
        return { x: spec.cx - spec.l, y: spec.cy - spec.l, w: spec.l * 2, h: spec.l * 2 };
    if (spec.kind === "paddle")
        return { x: spec.x - spec.w / 2, y: spec.y, w: spec.w, h: spec.h };
    var s = Math.max(spec.w, spec.h);
    return { x: spec.x - s / 2, y: spec.y - s / 2, w: s, h: s };
}
