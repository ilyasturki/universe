.pragma library

// `api.system.controls` as rows, both looks: a range steps min..max, a choice lists its choices, a toggle is "on" or "off".

var ICONS = { brightness: "sun", refresh: "screen", tdp: "bolt", gpu: "gauge", fan: "snowflake" };

function icon(control) {
    return ICONS[control.id] || "sliders";
}

function values(control) {
    if (control.kind === "choice")
        return control.choices.slice();
    if (control.kind === "toggle")
        return ["on", "off"];
    var out = [];
    for (var v = control.min; v <= control.max; v += Math.max(1, control.step))
        out.push(String(v));
    return out;
}

function label(control, value) {
    var v = value === undefined ? control.value : String(value);
    if (control.kind === "toggle")
        return v === "on" ? "On" : "Off";
    if (v === "auto")
        return "Auto";
    return control.unit ? v + " " + control.unit : v;
}

function labels(control) {
    return values(control).map(function (v) {
        return label(control, v);
    });
}

// One step along the values, held at either end.
function stepped(control, value, dir) {
    var all = values(control);
    var i = all.indexOf(String(value));
    return all[Math.max(0, Math.min(all.length - 1, (i < 0 ? 0 : i) + dir))];
}
