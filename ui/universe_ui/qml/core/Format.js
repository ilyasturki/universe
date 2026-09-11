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

function sessions(count) {
    if (!count || count <= 0)
        return "";
    return count + (count === 1 ? " session" : " sessions");
}

function clock() {
    var d = new Date();
    return ("0" + d.getHours()).slice(-2) + ":" + ("0" + d.getMinutes()).slice(-2);
}
