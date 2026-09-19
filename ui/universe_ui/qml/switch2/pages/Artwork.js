.pragma library

function search(shell, form) {
    shell.prompt({ title: "Search SteamGridDB", value: form.title }, function(query) {
        if (query === null)
            return;
        var settled = function() {
            if (form.searchBusy)
                return;
            form.hitsChanged.disconnect(settled);
            if (form.searchError !== "") {
                shell.showToast(form.searchError);
                return;
            }
            var hits = form.hits;
            if (hits.length === 0) {
                shell.showToast("Nothing on SteamGridDB matched “" + query + "”");
                return;
            }
            var names = hits.map(function(h) { return h.name + (h.year > 0 ? " (" + h.year + ")" : "") + (h.verified ? " ✓" : ""); });
            var current = hits.findIndex(function(h) { return h.current; });
            shell.pick({ title: "Which game is it on SteamGridDB?", choices: names, index: Math.max(0, current) }, function(i) {
                if (i >= 0)
                    form.pin(hits[i].id);
            });
        };
        form.hitsChanged.connect(settled);
        form.search(query);
    });
}
