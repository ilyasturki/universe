.pragma library

// A group's rows from `divider` on are its advanced ones, each folded card under a heading of its own (`dividers`).
function ruleAt(g, k) {
    if (g.dividers !== undefined && g.dividers.length > 0) {
        var d = g.dividers.filter(function(d) { return d.at === k; })[0];
        return d ? d.label : "";
    }
    return g.divider !== undefined && g.divider >= 0 && k === g.divider ? "Advanced" : "";
}

// A group's title opens a sub-section (`part`); an Advanced rule is a caption inside one.
function grouped(groups, rows, make) {
    var out = [];
    groups.forEach(function(g) {
        if (g.title)
            out.push({ heading: true, part: true, label: g.title, display: g.meta || "", changed: g.changed === true });
        g.rows.forEach(function(i, k) {
            var rule = ruleAt(g, k);
            if (rule !== "")
                out.push({ heading: true, label: rule, display: "" });
            var r = make(rows[i], i, g);
            if (r.key === "advanced")
                r.icon = "settings";
            out.push(r);
        });
    });
    return out;
}

// The rows cut at their sub-section headings: [{ label, detail, changed, rows }]; rows ahead of the first heading go under `first`.
function parts(model, first) {
    var out = [];
    model.forEach(function(r) {
        if (r.heading && r.part) {
            out.push({ label: r.label, detail: r.display || "", changed: r.changed === true, rows: [] });
            return;
        }
        if (out.length === 0)
            out.push({ label: first, detail: "", changed: false, rows: [] });
        out[out.length - 1].rows.push(r);
    });
    return out.length > 0 ? out : [{ label: first, detail: "", changed: false, rows: [] }];
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
