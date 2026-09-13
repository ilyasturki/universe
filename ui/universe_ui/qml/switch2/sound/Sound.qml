pragma Singleton
import QtQuick
import "../../sound" as Base

Base.SoundPool {
    dir: Qt.resolvedUrl("../assets/sounds/")
    poolSizes: ({ tick: 4, type: 4, edge: 3, ok: 2, back: 2, select: 2, open: 1, home: 1, launch: 1 })

    function tick() { play("tick") }
    function ok() { play("ok") }
    function back() { play("back") }
    function edge() { play("edge") }
    function type() { play("type") }
    function select() { play("select") }
    function open() { play("open") }
    function home() { play("home") }
    function launch() { play("launch") }
}
