.pragma library

var ICONS = {
    volume_up: "volume-up", volume_down: "volume-down", mute: "mute", screenshot: "camera",
    mangohud: "gauge", stop: "stop", keys: "keyboard", command: "terminal"
};

function icon(action) { return ICONS[action] || ""; }
