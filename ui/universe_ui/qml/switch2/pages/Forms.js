.pragma library

function grouped(groups, rows, make) {
    var out = [];
    groups.forEach(function(g) {
        if (g.title)
            out.push({ heading: true, label: g.title, display: g.meta || "" });
        g.rows.forEach(function(i) {
            var r = make(rows[i], i, g);
            // The Advanced row's glyph, in this look's set.
            if (r.key === "advanced")
                r.icon = "settings";
            out.push(r);
        });
    });
    return out;
}

// A map row (launch.env, launch.dll_overrides): the menu lists its entries; "Add" asks a name then a value; an entry changes or goes.
function editMap(shell, row, apply) {
    var entries = row.entries || [];
    var noun = row.key === "launch.dll_overrides" ? "an override" : "a variable";
    var items = entries.map(function(e) { return { label: e.name + " = " + e.value, act: "entry:" + e.name }; });
    items.push({ label: "Add " + noun + "…", act: "add" });
    shell.menu(row.label, items, function(act) {
        if (act === "add") {
            shell.prompt({ title: "Name of " + noun, value: "", max: 64 }, function(name) {
                name = String(name || "").trim().replace(/[^A-Za-z0-9_\-]/g, "");
                if (name === "")
                    return;
                shell.prompt({ title: "Value of " + name, value: "" }, function(value) { if (value !== null) apply(name, value); });
            });
            return;
        }
        var name = act.substring(6);
        var current = entries.filter(function(e) { return e.name === name; })[0];
        shell.menu(name, [ { label: "Change the value", act: "value" }, { label: "Remove " + name, act: "remove" } ], function(next) {
            if (next === "value")
                shell.prompt({ title: "Value of " + name, value: current ? current.value : "" }, function(value) { if (value !== null) apply(name, value); });
            else if (next === "remove")
                apply(name, "");
        });
    });
}

// The first row after `index` that is not a heading: where the cursor goes once the Advanced row opens.
function firstAfter(model, index) {
    for (var i = index + 1; i < model.length; i++)
        if (!model[i].heading)
            return i;
    return index;
}

// The row whose form index is `formIndex`, or -1.
function rowOf(model, formIndex) {
    for (var i = 0; i < model.length; i++)
        if (model[i].form === formIndex)
            return i;
    return -1;
}
