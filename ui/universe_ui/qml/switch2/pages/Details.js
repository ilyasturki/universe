.pragma library

var SENTENCES = {
    "capture.enabled": "Record every session of this game with gpu-screen-recorder.",
    "capture.cursor": "Draw the cursor in the recording.",
    "capture.codec": "The video codec of the recordings; AV1 keeps files small on a GPU that encodes it.",
    "capture.fps": "Frames per second; auto follows the screen's refresh rate.",
    "capture.microphone": "Mix the microphone into the recording.",
    "capture.min_duration_s": "Sessions shorter than this are not kept.",
    "journal.enabled": "Write a journal entry after each session of this game, from its recording and screenshots.",
    "journal.language": "The language the entries are written in.",
    "journal.provider": "The model provider that writes the entries.",
    "journal.model": "The model asked for the entry.",
    "journal.markdown_export": "Also render each entry as a Markdown note.",
    "journal.journal_root": "Where the Markdown notes go.",
    "gog.games_dir": "Where GOG installs games; empty, the library's games folder.",
    "gog.scan_dirs": "Folders scanned for GOG installs, comma-separated; empty, the install folder.",
    "gog.platform": "The depot gogdl downloads: Windows builds run through Proton.",
    "gog.with_dlcs": "Install the DLCs you own along with the game.",
    "gog.auth_path": "The token file gogdl writes at login.",
    "gog.install_timeout_s": "An install or update longer than this is stopped.",
    "desktop.hide_cursor": "Hide the desktop cursor while the game runs.",
    "favorite": "Kept in the Favourites group of All Software.",
    "hidden": "Left out of every list; the CLI still sees it.",
    "sort_title": "The title lists sort by, when it differs from the shown one.",
    "tags": "Comma-separated; each tag is a group in All Software.",
    "metadata.sgdb_id": "The SteamGridDB game the artwork comes from.",
    "metadata.rawg_id": "The RAWG game the description comes from."
};

function sentence(module, key) {
    return SENTENCES[(module ? module + "." : "") + key] || SENTENCES[key] || "";
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
