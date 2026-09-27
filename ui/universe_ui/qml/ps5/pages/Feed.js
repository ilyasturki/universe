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
