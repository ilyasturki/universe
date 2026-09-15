.pragma library

// A family the table does not know is drawn with Xbox names.
var STYLE = {
    "dualsense-edge": "sony", "dualsense": "sony", "dualshock4": "sony",
    "xbox": "xbox", "xbox-elite": "xbox", "generic": "xbox",
    "switch-pro": "nintendo", "8bitdo-pro-3": "eightbitdo"
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
                  guide: ["", "home"], ls: ["L3"], rs: ["R3"] }
};

var EXTRA = {
    fn_left: ["Fn"], fn_right: ["Fn"], paddle_left: ["LB"], paddle_right: ["RB"],
    paddle_p1: ["P1"], paddle_p2: ["P2"], paddle_p3: ["P3"], paddle_p4: ["P4"],
    paddle_l4: ["L4"], paddle_r4: ["R4"], paddle_pl: ["PL"], paddle_pr: ["PR"],
    share: ["", "share"], capture: ["", "capture"], mute: ["", "mic"], star: ["", "star"]
};

function style(family) { return STYLE[family] || "xbox"; }

function shape(slot) {
    if (slot.indexOf("dpad") === 0)
        return "dpad";
    if (slot === "lb" || slot === "rb")
        return "bumper";
    if (slot === "lt" || slot === "rt")
        return "trigger";
    if (slot === "ls" || slot === "rs")
        return "stick";
    if (slot === "select" || slot === "start" || slot === "share" || slot === "capture" || slot === "mute" || slot === "star")
        return "small";
    if (slot.indexOf("fn_") === 0 || slot === "paddle_l4" || slot === "paddle_r4")
        return "tab";
    if (slot.indexOf("paddle_") === 0)
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

// The hint bar names buttons the Xbox way; the pad connected decides what is drawn.
var HINT_SLOTS = {
    A: "south", B: "east", X: "west", Y: "north", LB: "lb", RB: "rb", LT: "lt", RT: "rt",
    LS: "ls", RS: "rs", Start: "start", Select: "select", dpad: "dpad"
};

function hintSlot(name) { return HINT_SLOTS[name] || name; }
