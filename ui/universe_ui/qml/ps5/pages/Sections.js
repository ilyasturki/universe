.pragma library

// Settings' sections in order; the searches index the same entries. `first` names a section's first part; `page`, a
// section that is one page, opens it straight away.
var list = [
    { id: "search", label: "Search Settings", icon: "search", first: "Search" },
    { id: "launch", label: "Launch", icon: "rocket" },
    { id: "runners", label: "Runners", icon: "chip" },
    { id: "controllers", label: "Controllers", icon: "gamepad", page: "pages/ControllersPage.qml" },
    { id: "network", label: "Network", icon: "globe", first: "Settings" },
    { id: "sources", label: "Sources", icon: "cloud" },
    { id: "updates", label: "Updates", icon: "download" },
    { id: "modules", label: "Modules", icon: "puzzle" },
    { id: "artwork", label: "Artwork", icon: "image", first: "Library Artwork" },
    { id: "themes", label: "Themes", icon: "palette", first: "Look" },
    { id: "bluetooth", label: "Accessories", icon: "bluetooth", first: "Bluetooth Accessories" },
    { id: "sound", label: "Sound", icon: "sound", first: "Audio Output" },
    { id: "performance", label: "Performance", icon: "bolt", detail: "Brightness, refresh, power limit, GPU clock and fan" },
    { id: "doctor", label: "Doctor", icon: "doctor", first: "Checks" },
    { id: "about", label: "About", icon: "info", first: "System Information" }
];

// Sections folded into another: a landing on one opens the other.
var aliases = { components: "runners" };

// The sections `api.system` leaves: under Steam's Game Mode sound is Steam's, and Performance needs a control; Network and
// Accessories need NetworkManager's Wi-Fi card and BlueZ's adapter.
function shown(system) {
    return list.filter(function (s) {
        return !(s.id === "sound" && system.steam) && !(s.id === "performance" && system.controls.length === 0) && !(s.id === "network" && !system.network) && !(s.id === "bluetooth" && !system.bluetooth);
    });
}

// What the settings search indexes.
function forSearch(system) {
    return shown(system).map(function (s) {
        return { id: s.id, label: s.label, icon: s.icon };
    });
}
