.pragma library

function grouped(groups, rows, make) {
    var out = [];
    groups.forEach(function(g) {
        if (g.title)
            out.push({ heading: true, label: g.title, display: g.meta || "" });
        g.rows.forEach(function(i) { out.push(make(rows[i], i, g)); });
    });
    return out;
}
