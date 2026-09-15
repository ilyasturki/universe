.pragma library

function playTime(seconds) {
    if (!seconds || seconds <= 0)
        return "";
    if (seconds < 3600)
        return Math.max(1, Math.round(seconds / 60)) + " min";
    return (seconds / 3600).toFixed(1) + " h";
}

function totalPlayTime(seconds) {
    if (!seconds || seconds <= 0)
        return "0 h";
    return Math.round(seconds / 3600) + " h";
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

function sessions(count) {
    return count > 0 ? plural(count, "session", "sessions") : "";
}

// 1:02:03, or 4:05 under an hour: a player's counter.
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
