.pragma library

var BADGES = {
    "windows": "PC", "linux": "PC", "mac": "MAC", "macos": "MAC", "steam": "PC", "dos": "DOS", "ms-dos": "DOS", "scummvm": "PC",
    "nintendo switch": "SWITCH", "nintendo wii": "WII", "nintendo wii u": "WII U", "nintendo gamecube": "GC",
    "nintendo ds": "DS", "nintendo 3ds": "3DS", "nintendo 64": "N64", "nintendo snes": "SNES", "super nintendo": "SNES",
    "nintendo game boy": "GB", "nintendo game boy advance": "GBA",
    "sony playstation": "PS1", "playstation": "PS1", "sony playstation 2": "PS2", "sony playstation 3": "PS3",
    "sony playstation 4": "PS4", "sony playstation 5": "PS5", "sony playstation portable": "PSP", "psp": "PSP",
    "sony playstation vita": "VITA", "ps vita": "VITA",
    "xbox": "XBOX", "microsoft xbox": "XBOX", "microsoft xbox 360": "X360", "xbox 360": "X360",
    "sega dreamcast": "DC", "arcade": "ARCADE"
};

// The console's platform badge beside a title (PS5, PS4): the game's platform, short.
function badge(game) {
    if (!game)
        return "";
    var p = String(game.platform || "").toLowerCase();
    return BADGES[p] || (p.length > 0 && p.length <= 6 ? p.toUpperCase() : "");
}

// The picture a game's world is drawn from: its background, else a screenshot, else its banner, else its box cropped.
function art(game) {
    if (!game)
        return { source: "", cropped: false };
    if (String(game.assets.background) !== "")
        return { source: game.assets.background, cropped: false };
    var shots = game.assets.screenshotList;
    if (shots && shots.length > 0)
        return { source: shots[0], cropped: false };
    if (String(game.assets.banner) !== "")
        return { source: game.assets.banner, cropped: false };
    return { source: game.assets.boxFront, cropped: true };
}

// The hero's square side tile: a banner has its title baked in and loses it to the crop, so it comes last.
function sideArt(game) {
    if (!game)
        return "";
    var shots = game.assets.screenshotList || [];
    var background = String(game.assets.background);
    if (shots.length > 1 || (shots.length === 1 && background !== ""))
        return shots[shots.length - 1];
    if (background !== "")
        return background;
    if (String(game.assets.banner) !== "")
        return game.assets.banner;
    return game.assets.boxFront;
}

var PAGES = {
    info: "pages/SoftwareInfoPage.qml",
    trophies: "pages/AchievementsPage.qml",
    settings: "pages/GameSettingsPage.qml",
    gallery: "pages/MediaGalleryPage.qml",
    journal: "pages/NewsPage.qml",
    log: "pages/PlayLogPage.qml",
    data: "pages/DataPage.qml",
    artwork: "pages/ArtworkPage.qml"
};

// The game's "…" menu wherever it opens (hero, Library, Store card); the shell's gameOption() runs a row's `act`.
function options(game, playing) {
    var items = playing ? [
        { label: "Resume", glyph: "play", act: "resume" },
        { label: "Close Game", glyph: "stop", act: "close" }
    ] : [
        { label: game.playTime > 0 ? "Continue" : "Play", glyph: "play", act: "play" }
    ];
    items.push({ label: "Information", glyph: "info", act: "info" });
    if (game.achievementsTotal > 0)
        items.push({ label: "Trophies", glyph: "trophy", act: "trophies" });
    items.push(
        { label: "Game Settings", glyph: "sliders", act: "settings" },
        { label: "Media Gallery", glyph: "gallery", act: "gallery" },
        { label: "Journal", glyph: "journal", act: "journal" },
        { label: "Play Log", glyph: "clock", act: "log" },
        { label: "Saved Data and Storage", glyph: "storage", act: "data" },
        { label: "Artwork", glyph: "image", act: "artwork" },
        { label: "Favourite", glyph: "star", toggle: game.favorite, act: "favourite", gap: true },
        { label: "Remove from Library…", glyph: "trash", act: "remove", gap: true }
    );
    return items;
}

function tagline(game) {
    if (!game)
        return "";
    var s = String(game.summary || "");
    if (s !== "")
        return s;
    var bits = [];
    if (game.genreList && game.genreList.length > 0)
        bits.push(game.genreList.slice(0, 2).join(", "));
    if (game.developerList && game.developerList.length > 0)
        bits.push(game.developerList[0]);
    if (game.releaseYear > 0)
        bits.push(String(game.releaseYear));
    return bits.join(" · ");
}

function hours(seconds) {
    if (!seconds || seconds <= 0)
        return "";
    if (seconds < 3600)
        return Math.max(1, Math.round(seconds / 60)) + " min";
    return Math.round(seconds / 3600) + " h";
}

function date(text) {
    var d = new Date(text);
    return isNaN(d.getTime()) ? null : d;
}
