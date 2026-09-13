.pragma library

var SENTENCES = {
    "capture.enabled": "Record every session of this game with gpu-screen-recorder.",
    "capture.cursor": "Draw the cursor in the recording.",
    "capture.codec": "The video codec of the recordings; AV1 keeps files small on a GPU that encodes it.",
    "capture.fps": "Frames per second; auto follows the screen's refresh rate.",
    "capture.microphone": "Mix the microphone into the recording.",
    "capture.min_duration_s": "Sessions shorter than this are not kept.",
    "journal.enabled": "Write a journal entry after each session of this game.",
    "journal.language": "The language the entries are written in.",
    "journal.provider": "The model provider that writes the entries.",
    "journal.model": "The model asked for the entry.",
    "journal.markdown_export": "Also render each entry as a Markdown note.",
    "journal.journal_root": "Where the Markdown notes go.",
    "gog.games_dir": "Where GOG installs games.",
    "launch.proton": "The Proton build umu-run starts the game with.",
    "launch.esync": "Wine's eventfd synchronisation; faster, on for most games.",
    "launch.fsync": "Futex synchronisation; needs a kernel with futex2, faster than esync.",
    "launch.mangohud": "Show MangoHud's overlay in the game.",
    "launch.args": "Arguments appended to the game's command line.",
    "launch.working_dir": "The folder the game starts in.",
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
    var out = {};
    for (var k in row)
        out[k] = row[k];
    if (!out.detail)
        out.detail = sentence(module, row.key);
    return out;
}

function enabledSentence(name, kind) {
    if (kind && kind.indexOf("source") >= 0)
        return "Games from " + name + " can be installed and updated.";
    return name + " runs its hooks around every session.";
}
