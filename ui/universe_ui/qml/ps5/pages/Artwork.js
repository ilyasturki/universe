.pragma library

// SteamGridDB's entries for the game's own title first; the last choice takes another name and searches again.
function search(shell, form, query) {
    if (query === undefined)
        query = form.title;
    var settled = function() {
        if (form.searchBusy)
            return;
        form.hitsChanged.disconnect(settled);
        if (form.searchError !== "") {
            shell.showToast(form.searchError);
            return;
        }
        var hits = form.hits;
        var names = hits.map(function(h) { return h.name + (h.year > 0 ? " (" + h.year + ")" : "") + (h.verified ? " ✓" : ""); });
        names.push("Another name…");
        var current = hits.findIndex(function(h) { return h.current; });
        var title = hits.length > 0 ? "Which game is it on SteamGridDB?" : "Nothing on SteamGridDB matched “" + query + "”";
        shell.pick({ title: title, choices: names, index: Math.max(0, current) }, function(i) {
            if (i < 0)
                return;
            if (i < hits.length) {
                form.pin(hits[i].id);
                return;
            }
            shell.prompt({ title: "Search SteamGridDB", value: query }, function(next) {
                if (next !== null && next !== "")
                    search(shell, form, next);
            });
        });
    };
    form.hitsChanged.connect(settled);
    form.search(query);
}
