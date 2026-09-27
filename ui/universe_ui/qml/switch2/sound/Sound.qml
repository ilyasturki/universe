pragma Singleton
import QtQuick
import "../../sound" as Base

Base.SoundPool {
    dir: Qt.resolvedUrl("../assets/sounds/")
    // The singleton outlives a switch to another look, whose folder is not ours.
    overrides: api.theme.current === "switch2" ? api.theme.soundFiles : ({})
    poolSizes: ({
            tick: 4,
            "tick-side": 4,
            "tick-tile": 4,
            type: 4,
            edge: 3,
            ok: 2,
            back: 2,
            select: 2,
            deselect: 2,
            tab: 2,
            open: 1,
            home: 1,
            launch: 1,
            "icon-grid": 1,
            "icon-news": 1,
            "icon-shop": 1,
            "icon-album": 1,
            "icon-controllers": 1,
            "icon-settings": 1,
            "icon-power": 1
        })
}
