.pragma library

// The Settings sidebar in order; the tab bar's search indexes the same entries.
var list = [
    {
        id: "launch",
        name: "Launch",
        icon: "sliders"
    },
    {
        id: "runners",
        name: "Runners",
        icon: "play"
    },
    {
        id: "controller",
        name: "Controller",
        icon: "gamepad"
    },
    {
        id: "sources",
        name: "Sources",
        icon: "cloud"
    },
    {
        id: "install",
        name: "Install",
        icon: "download"
    },
    {
        id: "modules",
        name: "Modules",
        icon: "grid"
    },
    {
        id: "artwork",
        name: "Artwork",
        icon: "image"
    },
    {
        id: "themes",
        name: "Themes",
        icon: "sun"
    },
    {
        id: "network",
        name: "Network",
        icon: "wifi"
    },
    {
        id: "bluetooth",
        name: "Bluetooth",
        icon: "bluetooth"
    },
    {
        id: "sound",
        name: "Sound",
        icon: "volume-up"
    },
    {
        id: "system",
        name: "System",
        icon: "bolt"
    },
    {
        id: "storage",
        name: "Storage",
        icon: "folder"
    },
    {
        id: "doctor",
        name: "Doctor",
        icon: "pulse"
    },
    {
        id: "about",
        name: "About",
        icon: "info"
    }
];

// Sections folded into another: a landing on one opens the other.
var aliases = {
    components: "runners",
    updates: "install",
    quit: "about",
    power: "about"
};

// The sections `api.system` leaves: under Steam's Game Mode sound is Steam's, System needs a control to show, Network and
// Bluetooth the machine's NetworkManager and adapter.
function shown(system) {
    return list.filter(function (s) {
        return !(s.id === "sound" && system.steam) && !(s.id === "system" && system.controls.length === 0) && !(s.id === "network" && !system.network) && !(s.id === "bluetooth" && !system.bluetooth);
    });
}
