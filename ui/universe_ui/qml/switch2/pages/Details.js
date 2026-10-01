.pragma library

var SENTENCES = {
    "desktop.hide_cursor": "Hide the desktop cursor while the game runs.",
    "favorite": "Kept in the Favourites group of All Software.",
    "hidden": "Left out of every list; the CLI still sees it.",
    "sort_title": "The title lists sort by, when it differs from the shown one.",
    "tags": "Comma-separated; each tag is a group in All Software.",
    "metadata.steam_appid": "The Steam game the art and the description come from.",
    "metadata.gamesdb_id": "The GOG GamesDB game the art and the description come from.",
    "metadata.sgdb_id": "The SteamGridDB game the artwork comes from, with your key."
};

// A module's or a source's row carries its manifest's description; these are the core's own rows.
function sentence(module, key) {
    return module ? "" : SENTENCES[key] || "";
}

function withDetail(row, module) {
    var out = Object.assign({}, row);
    if (!out.detail)
        out.detail = sentence(module, row.key);
    return out;
}

function enabledSentence(name, source) {
    if (source)
        return "Games from " + name + " can be installed and updated.";
    return name + " runs its hooks around every session.";
}
