.pragma library

function listOf(model) {
    var out = [];
    if (!model)
        return out;
    for (var i = 0; i < model.count; i++) {
        var g = model.get(i);
        if (g)
            out.push(g);
    }
    return out;
}

function tagged(games, tag) {
    return games.filter(function(g) { return g.tags.indexOf(tag) !== -1; });
}

function tags(games) {
    var seen = {}, out = [];
    games.forEach(function(g) {
        g.tags.forEach(function(t) {
            if (t !== "" && !seen[t]) {
                seen[t] = true;
                out.push(t);
            }
        });
    });
    out.sort(function(a, b) { return a.toLowerCase() < b.toLowerCase() ? -1 : 1; });
    return out;
}

function groups(all, collections) {
    var games = listOf(all);
    var out = [{ key: "favourites", name: "Favourites", games: games.filter(function(g) { return g.favorite; }) }];
    for (var i = 0; i < collections.count; i++) {
        var c = collections.get(i);
        if (c)
            out.push({ key: "platform:" + c.id, name: c.name, games: listOf(c.games) });
    }
    tags(games).forEach(function(t) {
        out.push({ key: "tag:" + t, name: t.charAt(0).toUpperCase() + t.slice(1), games: tagged(games, t) });
    });
    return out;
}

function gamesOf(all, collections, key) {
    if (key.indexOf("platform:") === 0) {
        var c = listOf(collections).find(function(c) { return "platform:" + c.id === key; });
        return c ? listOf(c.games) : [];
    }
    var games = listOf(all);
    return key === "favourites" ? games.filter(function(g) { return g.favorite; }) : tagged(games, key.slice(4));
}
