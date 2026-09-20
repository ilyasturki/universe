pragma Singleton
import QtQuick

SoundPool {
    dir: Qt.resolvedUrl("../assets/sounds/")
    // Restarting a playing SoundEffect rebuilds its pulse stream and clicks, so a burst spreads over voices.
    poolSizes: ({
            tick: 4,
            kbtick: 4,
            type: 4,
            backspace: 4,
            edge: 3,
            panel: 3,
            collection: 3,
            sort: 3,
            space: 2,
            enter: 1,
            cancel: 1,
            launch: 1,
            "favourite-on": 1,
            "favourite-off": 1
        })

    function tick() {
        play("tick");
    }
    function kbtick() {
        play("kbtick");
    }
    function panel() {
        play("panel");
    }
    function space() {
        play("space");
    }
    function enter() {
        play("enter");
    }
    function cancel() {
        play("cancel");
    }
    function edge() {
        play("edge");
    }
    function collection() {
        play("collection");
    }
    function sort() {
        play("sort");
    }
    function favourite(on) {
        play(on ? "favourite-on" : "favourite-off");
    }
    function launch() {
        play("launch");
    }
    function type() {
        play("type");
    }
    function backspace() {
        play("backspace");
    }
}
