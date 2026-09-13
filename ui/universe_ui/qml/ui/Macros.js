.pragma library

// The menu glyph for a macro's action: a preset by its id, a key combo, a command.
var ICONS = {
    volume_up: "volume-up", volume_down: "volume-down", mute: "mute", screenshot: "camera",
    mangohud: "gauge", stop: "stop", keys: "keyboard", command: "terminal"
};

function icon(action) { return ICONS[action] || ""; }
