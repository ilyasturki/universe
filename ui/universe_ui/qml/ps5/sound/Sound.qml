pragma Singleton
import QtQuick
import "../../sound" as Base

Base.SoundPool {
    dir: Qt.resolvedUrl("../assets/sounds/")
    // The singleton outlives a switch to another look, whose folder is not ours.
    overrides: api.theme.current === "ps5" ? api.theme.soundFiles : ({})
    poolSizes: ({
            tick: 4,
            type: 4,
            edge: 3,
            ok: 2,
            back: 2,
            select: 2,
            open: 1,
            panel: 1,
            home: 1,
            launch: 1
        })
}
