.pragma library

// Settings' sections in order, under their group headers; the searches index the same entries. `first` names a section's
// first part; `page`, a section that is one page, opens it straight away.
var list = [
    { id: "search", label: "Search Settings", icon: "search", first: "Search" },
    { id: "sources", label: "Stores", icon: "cloud", group: "Games" },
    { id: "updates", label: "Updates", icon: "download", group: "Games" },
    { id: "controllers", label: "Controllers", icon: "gamepad", page: "pages/ControllersPage.qml", group: "Games" },
    { id: "launch", label: "Game Defaults", icon: "rocket", group: "Games" },
    { id: "runners", label: "Emulators & Proton", icon: "chip", group: "Games" },
    { id: "modules", label: "Features", icon: "puzzle", group: "Extras" },
    { id: "artwork", label: "Artwork", icon: "image", first: "Library Artwork", group: "Extras" },
    { id: "themes", label: "Appearance", icon: "palette", first: "Look", group: "Extras" },
    { id: "network", label: "Network", icon: "globe", first: "Settings", group: "System" },
    { id: "bluetooth", label: "Accessories", icon: "bluetooth", first: "Bluetooth Accessories", group: "System" },
    { id: "sound", label: "Sound", icon: "sound", first: "Audio Output", group: "System" },
    { id: "performance", label: "Performance", icon: "bolt", detail: "Brightness, refresh, power limit, GPU clock and fan", group: "System" },
    { id: "doctor", label: "System Check", icon: "doctor", first: "Checks", group: "Universe" },
    { id: "about", label: "About", icon: "info", first: "System Information", group: "Universe" }
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
