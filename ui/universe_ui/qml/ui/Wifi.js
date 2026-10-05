.pragma library

// What A on a network row calls for: "refuse" (a kind Universe does not set up), "forget" (the one joined), "password" (a
// secured one not saved yet), else "join".
function step(wifi, row) {
    if (row.active)
        return "forget";
    if (!row.joinable)
        return "refuse";
    return wifi.needsPassword(row.ssid) ? "password" : "join";
}

var SECURITY = {
    open: "Open",
    owe: "Open, encrypted",
    psk: "WPA2",
    sae: "WPA3",
    wep: "WEP",
    enterprise: "WPA Enterprise"
};

function refusal(row) {
    return "Universe does not set up " + (SECURITY[row.security] || row.security) + " networks: join " + row.ssid + " once from a desktop";
}

function passwordTitle(ssid) {
    return "Password for " + ssid;
}

function joined(ssid, connectivity) {
    return "Connected to " + ssid + (connectivity === "portal" ? " · the network wants a sign-in" : connectivity === "limited" || connectivity === "none" ? " · no internet yet" : "");
}

function connectivity(state) {
    return ({
            full: "The internet answers",
            limited: "Connected, the internet does not answer",
            portal: "The network wants a sign-in first",
            none: "No internet"
        })[state] || "Not checked yet";
}

function link(wifi) {
    return wifi.link === "wired" ? "Cable" : wifi.link === "wifi" ? wifi.ssid : "Offline";
}

function signal(bars) {
    return ["", "Weak signal", "Fair signal", "Strong signal"][bars] || "";
}

function row(n, connecting) {
    var busy = connecting === n.ssid;
    return {
        section: "Network",
        key: "network",
        label: n.ssid,
        type: "action",
        display: n.active ? "Connected" : busy ? "Connecting…" : n.saved ? "Saved" : "",
        detail: [signal(n.bars), SECURITY[n.security] || n.security].filter(Boolean).join(" · "),
        icon: n.secured ? "lock" : "wifi",
        accent: n.active,
        disabled: !n.joinable && !n.active,
        action: n.active ? "Forget" : !n.joinable ? "" : "Connect",
        ssid: n.ssid,
        saved: n.saved,
        active: n.active,
        secured: n.secured,
        joinable: n.joinable,
        bars: n.bars,
        security: n.security
    };
}
