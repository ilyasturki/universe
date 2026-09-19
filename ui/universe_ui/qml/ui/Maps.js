.pragma library

// A map row (launch.env, launch.dll_overrides) edits one entry at a time: the menu lists them, "Add" asks a name then a value.

function entries(row) {
    return row.entries || [];
}

function noun(row) {
    return row.key === "launch.dll_overrides" ? "an override" : "a variable";
}

function items(row) {
    var out = entries(row).map(function(e) { return { icon: "sliders", label: e.name + " = " + e.value, action: "entry:" + e.name }; });
    out.push({ icon: "plus", label: "Add " + noun(row) + "…", action: "add" });
    return out;
}

function entryItems(name) {
    return [ { icon: "keyboard", label: "Change the value", action: "value" }, { icon: "trash", label: "Remove " + name, action: "remove", danger: true } ];
}

function valueOf(row, name) {
    var e = entries(row).filter(function(e) { return e.name === name; })[0];
    return e ? e.value : "";
}

// A name as one key of the map: letters, digits, underscores and dashes (a dot would nest a table).
function cleanName(text) {
    return String(text || "").trim().replace(/[^A-Za-z0-9_\-]/g, "");
}
