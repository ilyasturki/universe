.pragma library

function channels(rows) {
    var seen = {}, out = [];
    for (var i = 0; i < rows.length; i++) {
        var id = rows[i].gameId;
        if (!seen[id]) {
            seen[id] = { id: id, title: rows[i].gameTitle, count: 0 };
            out.push(seen[id]);
        }
        seen[id].count++;
    }
    return out;
}

// An entry nobody has written yet: never asked for, put off after a failure, or given up on.
function blank(row) {
    return row.state === "none" || row.state === "deferred" || row.state === "failed";
}

function heading(row) {
    if (row.state === "pending")
        return "Writing the entry…";
    if (row.state === "none")
        return "No entry yet";
    return row.title;
}
