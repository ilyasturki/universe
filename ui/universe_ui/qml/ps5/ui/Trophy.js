.pragma library
.import "../../core/Format.js" as Format

// The console's grades of rarity, by the share of players who earned it.
function grade(r) {
    return r < 5 ? "Ultra rare" : r < 15 ? "Very rare" : r < 50 ? "Rare" : "Common";
}

// "Rare · 12%"; "" while the store gives no share. `r` is an achievements row's `rarity`, -1 when unknown.
function rarity(r) {
    if (r === undefined || r === null || r < 0)
        return "";
    return grade(r) + "  ·  " + (r < 10 ? r.toFixed(1) : Math.round(r)) + "%";
}

// When it was earned, as the console says it: "Today", "3 days ago". `unlockedAt` is the row's time as the store gave
// it, not always ISO: one JS cannot read falls back to the row's `dateText`.
function earned(unlockedAt, dateText) {
    var at = new Date(unlockedAt || "");
    return isNaN(at.getTime()) || at.getFullYear() <= 1971 ? dateText || "" : Format.lastPlayed(at);
}
