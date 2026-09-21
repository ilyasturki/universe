.pragma library

// The test screen's log, newest first: { slot, label, dt }, dt the milliseconds since the entry before (null on the first).

var MAX = 12;
var ARM = 0.5;
var REARM = 0.2;

var DIRECTIONS = ["right", "down-right", "down", "down-left", "left", "up-left", "up", "up-right"];

function fresh() {
    return { entries: [], last: 0, open: {}, axes: {} };
}

function log(state, slot, label, now) {
    var entry = { slot: slot, label: label, dt: state.last > 0 ? now - state.last : null };
    state.last = now;
    state.entries = [entry].concat(state.entries).slice(0, MAX);
    return state.entries;
}

// A press: logged. Returns the new entries, or null when nothing was logged.
function press(state, slot, down, label, now) {
    return down ? log(state, slot, label, now) : null;
}

// An axis move: a trigger logs its pull and a stick its direction once past half, the entry then following the peak
// until the axis comes back to rest.
function axis(state, name, value, labelOf, now) {
    state.axes[name] = value;
    var slot, level, detail;
    if (name === "lt" || name === "rt") {
        slot = name;
        level = value;
        detail = Math.round(value * 100) + " %";
    } else if (name === "lx" || name === "ly" || name === "rx" || name === "ry") {
        slot = name.charAt(0) + "s";
        var x = state.axes[name.charAt(0) + "x"] || 0, y = state.axes[name.charAt(0) + "y"] || 0;
        level = Math.sqrt(x * x + y * y);
        detail = DIRECTIONS[(Math.round(Math.atan2(y, x) / (Math.PI / 4)) + 8) % 8];
    } else {
        return null;
    }
    var open = state.open[slot];
    if (open) {
        if (level < REARM) {
            delete state.open[slot];
            return null;
        }
        if (level <= open.level)
            return null;
        open.level = level;
        open.entry.label = labelOf(slot) + " · " + detail;
        return state.entries.slice();
    }
    if (level < ARM)
        return null;
    var entries = log(state, slot, labelOf(slot) + " · " + detail, now);
    state.open[slot] = { entry: entries[0], level: level };
    return entries;
}
