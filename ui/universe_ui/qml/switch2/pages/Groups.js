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
    var hit = groups(all, collections).filter(function(g) { return g.key === key; })[0];
    return hit ? hit.games : [];
}

function count(n) {
    return n + (n === 1 ? " game" : " games");
}
