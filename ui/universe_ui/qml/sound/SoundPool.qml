import QtQuick
import QtMultimedia

QtObject {
    id: s

    property url dir
    property var poolSizes: ({})
    readonly property Component voice: Component { SoundEffect {} }
    readonly property var voices: ({})
    readonly property var lastPlayed: ({})

    function preload() {
        // The singleton outlives the tree: a theme switch back calls this again.
        if (Object.keys(voices).length > 0)
            return;
        for (var name in poolSizes) {
            var pool = [];
            for (var i = 0; i < poolSizes[name]; i++)
                pool.push({ fx: voice.createObject(s, { source: dir + name + ".wav" }), at: 0 });
            voices[name] = pool;
        }
    }

    function stepped(i, d, n) {
        var next = Math.max(0, Math.min(n - 1, i + d));
        play(next === i ? "edge" : "tick");
        return next;
    }

    function paged(i, d, columns, rows, n) {
        if (n <= 0)
            return i;
        var lastRow = Math.floor((n - 1) / columns);
        var row = Math.max(0, Math.min(lastRow, Math.floor(i / columns) + d * Math.max(1, rows)));
        var next = Math.min(n - 1, row * columns + i % columns);
        play(next === i ? "edge" : "tick");
        return next;
    }

    function play(name) {
        var now = Date.now();
        if (now - (lastPlayed[name] || 0) < 15)
            return;
        lastPlayed[name] = now;
        var pool = voices[name];
        if (!pool)
            return;
        var pick = pool[0];
        for (var i = 0; i < pool.length; i++) {
            if (!pool[i].fx.playing) {
                pick = pool[i];
                break;
            }
            if (pool[i].at < pick.at)
                pick = pool[i];
        }
        pick.at = now;
        if (pick.fx.status === SoundEffect.Ready)
            pick.fx.play();
    }
}
