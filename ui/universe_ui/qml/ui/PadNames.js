.pragma library

var STYLE = {
    "dualsense-edge": "sony", "dualsense": "sony", "dualshock4": "sony",
    "xbox": "xbox", "xbox-elite": "xbox", "generic": "xbox",
    "switch-pro": "nintendo", "8bitdo-pro-3": "eightbitdo", "steam-deck": "deck"
};

// slot → [text, symbol]
var FACE = {
    sony: { south: ["", "cross"], east: ["", "circle"], north: ["", "triangle"], west: ["", "square"],
            lb: ["L1"], rb: ["R1"], lt: ["L2"], rt: ["R2"], select: ["", "create"], start: ["", "menu"],
            guide: ["PS"], ls: ["L3"], rs: ["R3"] },
    xbox: { south: ["A"], east: ["B"], north: ["Y"], west: ["X"],
            lb: ["LB"], rb: ["RB"], lt: ["LT"], rt: ["RT"], select: ["", "view"], start: ["", "menu"],
            guide: ["", "xbox"], ls: ["LS"], rs: ["RS"] },
    nintendo: { south: ["B"], east: ["A"], north: ["X"], west: ["Y"],
                lb: ["L"], rb: ["R"], lt: ["ZL"], rt: ["ZR"], select: ["", "minus"], start: ["", "plus"],
                guide: ["", "home"], ls: ["LS"], rs: ["RS"] },
    eightbitdo: { south: ["B"], east: ["A"], north: ["X"], west: ["Y"],
                  lb: ["L1"], rb: ["R1"], lt: ["L2"], rt: ["R2"], select: ["", "minus"], start: ["", "plus"],
                  guide: ["", "home"], ls: ["L3"], rs: ["R3"] },
    deck: { south: ["A"], east: ["B"], north: ["Y"], west: ["X"],
            lb: ["L1"], rb: ["R1"], lt: ["L2"], rt: ["R2"], select: ["", "view"], start: ["", "menu"],
            guide: ["", "steam"], ls: ["L3"], rs: ["R3"] }
};

var EXTRA = {
    fn_left: ["Fn"], fn_right: ["Fn"], paddle_left: ["LB"], paddle_right: ["RB"],
    paddle_p1: ["P1"], paddle_p2: ["P2"], paddle_p3: ["P3"], paddle_p4: ["P4"],
    paddle_l4: ["L4"], paddle_r4: ["R4"], paddle_pl: ["PL"], paddle_pr: ["PR"],
    share: ["", "share"], capture: ["", "capture"], mute: ["", "mic"], star: ["", "star"],
    grip_l4: ["L4"], grip_l5: ["L5"], grip_r4: ["R4"], grip_r5: ["R5"], quick: ["", "more"],
    pad_left: ["", "pad"], pad_right: ["", "pad"]
};

function style(family) { return STYLE[family] || "xbox"; }

function shape(slot) {
    if (slot.indexOf("dpad") === 0)
        return "dpad";
    if (slot === "lb" || slot === "rb" || slot === "paddle_l4" || slot === "paddle_r4")
        return "bumper";
    if (slot === "lt" || slot === "rt")
        return "trigger";
    if (slot === "ls" || slot === "rs")
        return "stick";
    if (slot === "select" || slot === "start" || slot === "share" || slot === "capture" || slot === "mute" || slot === "star" || slot === "quick")
        return "small";
    if (slot.indexOf("pad_") === 0)
        return "circle";
    if (slot.indexOf("fn_") === 0)
        return "tab";
    if (slot.indexOf("paddle_") === 0 || slot.indexOf("grip_") === 0)
        return "paddle";
    return "circle";
}

// { shape, text, symbol, dir } for one slot of one family; dir is the D-pad arm.
function glyph(family, slot) {
    var face = FACE[style(family)];
    var entry = face[slot] || EXTRA[slot] || [""];
    var out = { shape: shape(slot), text: entry[0] || "", symbol: entry[1] || "", dir: "" };
    if (out.shape === "dpad" && slot.length > 5)
        out.dir = slot.substring(5);
    if (out.text === "" && out.symbol === "" && out.shape !== "dpad" && slot !== "")
        out.text = slot.length > 3 ? slot.substring(0, 3) : slot;
    return out;
}

var SYMBOL_TEXT = { cross: "✕", circle: "○", triangle: "△", square: "□" };

// How copy names a hint's button on this family's pad: "A" on an Xbox pad, "✕" on a DualSense.
function buttonName(name, family) {
    var g = glyph(family, hintSlot(name, family));
    return g.text || SYMBOL_TEXT[g.symbol] || name;
}

var HINT_SLOTS = {
    A: "south", B: "east", X: "west", Y: "north", LB: "lb", RB: "rb", LT: "lt", RT: "rt",
    LS: "ls", RS: "rs", Start: "start", Select: "select", dpad: "dpad"
};
var FACE_SLOTS = ["south", "east", "north", "west"];

// A lettered hint lands on the button that carries the letter — the A on the right of a Nintendo-style pad — else on the position an Xbox pad has it.
function hintSlot(name, family) {
    var face = FACE[style(family || "xbox")];
    for (var i = 0; i < FACE_SLOTS.length; i++)
        if (face[FACE_SLOTS[i]][0] === name)
            return FACE_SLOTS[i];
    return HINT_SLOTS[name] || name;
}
