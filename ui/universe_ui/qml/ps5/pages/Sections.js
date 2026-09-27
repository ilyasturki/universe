.pragma library

// Settings' sections in order; the searches index the same entries. `first` names a section's first part.
var list = [
    { id: "search", label: "Search Settings", icon: "search", first: "Search" },
    { id: "launch", label: "Launch", icon: "rocket" },
    { id: "runners", label: "Runners", icon: "chip" },
    { id: "components", label: "Components", icon: "cube" },
    { id: "controllers", label: "Controllers", icon: "gamepad" },
    { id: "sources", label: "Sources", icon: "cloud" },
    { id: "updates", label: "Updates", icon: "download" },
    { id: "modules", label: "Modules", icon: "puzzle" },
    { id: "artwork", label: "Artwork", icon: "image", first: "Library Artwork" },
    { id: "themes", label: "Themes", icon: "palette", first: "Look" },
    { id: "sound", label: "Sound", icon: "sound", first: "Audio Output" },
    { id: "performance", label: "Performance", icon: "bolt", detail: "Brightness, refresh, power limit, GPU clock and fan" },
    { id: "doctor", label: "Doctor", icon: "doctor", first: "Checks" },
    { id: "about", label: "About", icon: "info", first: "System Information" }
];

// The sections `api.system` leaves: under Steam's Game Mode sound is Steam's, and Performance needs a control.
function shown(system) {
    return list.filter(function (s) {
        return !(s.id === "sound" && system.steam) && !(s.id === "performance" && system.controls.length === 0);
    });
}

// What the settings search indexes.
function forSearch(system) {
    return shown(system).map(function (s) {
        return { id: s.id, label: s.label, icon: s.icon };
    });
}
