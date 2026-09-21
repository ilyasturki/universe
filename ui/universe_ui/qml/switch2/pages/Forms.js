.pragma library

// A group's rows from `divider` on are its advanced ones, each folded card under a heading of its own (`dividers`).
function ruleAt(g, k) {
    if (g.dividers !== undefined && g.dividers.length > 0) {
        var d = g.dividers.filter(function(d) { return d.at === k; })[0];
        return d ? d.label : "";
    }
    return g.divider !== undefined && g.divider >= 0 && k === g.divider ? "Advanced" : "";
}

function grouped(groups, rows, make) {
    var out = [];
    groups.forEach(function(g) {
        if (g.title)
            out.push({ heading: true, label: g.title, display: g.meta || "" });
        g.rows.forEach(function(i, k) {
            var rule = ruleAt(g, k);
            if (rule !== "")
                out.push({ heading: true, label: rule, display: "" });
            var r = make(rows[i], i, g);
            // The Advanced row's glyph, in this look's set.
            if (r.key === "advanced")
                r.icon = "settings";
            out.push(r);
        });
    });
    return out;
}

// A map's add row: one sheet asks the name and the value together.
function addEntry(shell, row, apply) {
    shell.promptPair({ title: row.label.replace(/…$/, ""), labels: row.fields }, function(name, value) {
        if (name !== null)
            apply(name, value);
    });
}

// The first row after `index` that is not a heading, else the first row folded under an Advanced heading above:
// where the cursor goes once the Advanced row opens.
function firstAfter(model, index) {
    for (var i = index + 1; i < model.length; i++)
        if (!model[i].heading)
            return i;
    for (i = 1; i < model.length; i++)
        if (model[i - 1].heading && model[i - 1].label === "Advanced" && !model[i].heading)
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
