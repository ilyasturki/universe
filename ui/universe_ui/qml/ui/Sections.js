.pragma library

// The Settings sidebar in order, under its group headers; the tab bar's search indexes the same entries.
var list = [
    {
        id: "sources",
        name: "Stores",
        icon: "cloud",
        group: "Games"
    },
    {
        id: "install",
        name: "Downloads",
        icon: "download",
        group: "Games"
    },
    {
        id: "controller",
        name: "Controller",
        icon: "gamepad",
        group: "Games"
    },
    {
        id: "launch",
        name: "Game defaults",
        icon: "sliders",
        group: "Games"
    },
    {
        id: "runners",
        name: "Emulators & Proton",
        icon: "play",
        group: "Games"
    },
    {
        id: "modules",
        name: "Features",
        icon: "grid",
        group: "Extras"
    },
    {
        id: "artwork",
        name: "Artwork",
        icon: "image",
        group: "Extras"
    },
    {
        id: "themes",
        name: "Appearance",
        icon: "sun",
        group: "Extras"
    },
    {
        id: "network",
        name: "Network",
        icon: "wifi",
        group: "System"
    },
    {
        id: "bluetooth",
        name: "Bluetooth",
        icon: "bluetooth",
        group: "System"
    },
    {
        id: "sound",
        name: "Sound",
        icon: "volume-up",
        group: "System"
    },
    {
        id: "system",
        name: "Performance",
        icon: "bolt",
        group: "System"
    },
    {
        id: "storage",
        name: "Storage",
        icon: "folder",
        group: "System"
    },
    {
        id: "doctor",
        name: "System check",
        icon: "pulse",
        group: "Universe"
    },
    {
        id: "about",
        name: "About",
        icon: "info",
        group: "Universe"
    }
];

// Sections folded into another: a landing on one opens the other.
var aliases = {
    components: "runners",
    updates: "install",
    quit: "about",
    power: "about"
};

// The sections `api.system` leaves: under Steam's Game Mode sound is Steam's, Performance needs a control to show, Network
// and Bluetooth the machine's NetworkManager and adapter.
function shown(system) {
    return list.filter(function (s) {
        return !(s.id === "sound" && system.steam) && !(s.id === "system" && system.controls.length === 0) && !(s.id === "network" && !system.network) && !(s.id === "bluetooth" && !system.bluetooth);
    });
}
