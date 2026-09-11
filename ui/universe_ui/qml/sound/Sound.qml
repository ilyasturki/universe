pragma Singleton
import QtQuick
import QtMultimedia

QtObject {
    id: s

    property bool enabled: true
    // The shot probe sets this to log what fires; playback needs a sound server it lacks.
    property var trace: null

    // Restarting a playing SoundEffect rebuilds its pulse stream and clicks, so a burst spreads over voices.
    readonly property var poolSizes: ({
        tick: 4, kbtick: 4, type: 4, backspace: 4,
        edge: 3, panel: 3, collection: 3, sort: 3, space: 2,
        enter: 1, cancel: 1, launch: 1, "favourite-on": 1, "favourite-off": 1
    })
    readonly property Component voice: Component { SoundEffect {} }
    readonly property var voices: ({})
    readonly property var lastPlayed: ({})

    function preload() {
        for (var name in poolSizes) {
            var pool = [];
            for (var i = 0; i < poolSizes[name]; i++) {
                var url = Qt.resolvedUrl("../assets/sounds/" + name + ".wav");
                pool.push({ fx: voice.createObject(s, { source: url }), at: 0 });
            }
            voices[name] = pool;
        }
    }

    function play(name) {
        var now = Date.now();
        if (now - (lastPlayed[name] || 0) < 15)
            return;
        lastPlayed[name] = now;
        if (trace)
            trace(name);
        var pool = voices[name];
        if (!enabled || !pool)
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

    function tick() { play("tick") }
    function kbtick() { play("kbtick") }
    function panel() { play("panel") }
    function space() { play("space") }
    function enter() { play("enter") }
    function cancel() { play("cancel") }
    function edge() { play("edge") }
    function collection() { play("collection") }
    function sort() { play("sort") }
    function favourite(on) { play(on ? "favourite-on" : "favourite-off") }
    function launch() { play("launch") }
    function type() { play("type") }
    function backspace() { play("backspace") }
}
