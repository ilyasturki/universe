.pragma library
.import QtQuick as QQ

function playTime(seconds) {
    if (!seconds || seconds <= 0)
        return "";
    if (seconds < 3600)
        return Math.max(1, Math.round(seconds / 60)) + " min";
    return (seconds / 3600).toFixed(1) + " h";
}

function playLabel(game, playingId) {
    if (!game)
        return "Play";
    if (game.installing)
        return "Manage install";
    if (game.id === playingId)
        return "Resume";
    return game.playTime > 0 ? "Continue" : "Play";
}

function lastPlayed(date) {
    if (!(date instanceof Date) || isNaN(date.getTime()) || date.getFullYear() <= 1971)
        return "Never played";

    var days = Math.floor((Date.now() - date.getTime()) / 86400000);
    if (days <= 0)
        return "Today";
    if (days === 1)
        return "Yesterday";
    if (days < 30)
        return days + " days ago";
    if (days < 60)
        return "Last month";
    if (days < 365)
        return Math.floor(days / 30) + " months ago";
    if (days < 730)
        return "Last year";
    return Math.floor(days / 365) + " years ago";
}

function plural(n, one, many) {
    return n + " " + (n === 1 ? one : many);
}

// The host's `_size`, the same Qt call: 1024-based, one decimal past bytes, the C locale's spelling.
function bytes(n) {
    return Qt.locale("C").formattedDataSize(Math.max(0, n || 0), 1, QQ.Locale.DataSizeTraditionalFormat);
}

function sessions(count) {
    return count > 0 ? plural(count, "session", "sessions") : "";
}

function clockTime(seconds) {
    var s = Math.max(0, Math.floor(seconds || 0));
    var h = Math.floor(s / 3600);
    var m = Math.floor((s % 3600) / 60);
    var rest = ("0" + (s % 60)).slice(-2);
    return h > 0 ? h + ":" + ("0" + m).slice(-2) + ":" + rest : m + ":" + rest;
}

function clock() {
    return Qt.formatTime(new Date(), "HH:mm");
}
