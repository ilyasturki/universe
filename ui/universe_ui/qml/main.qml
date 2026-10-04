import QtQuick
import QtQuick.Window
import "ui"

Window {
    id: window

    width: 1920
    height: 1080
    visibility: api.fullscreen ? Window.FullScreen : Window.Windowed
    color: api.theme.ground
    title: "Universe"

    Loader {
        objectName: "look"
        anchors.fill: parent
        source: api.theme.entry
        focus: true
    }

    // Outside the Loader, so a theme switch never replays it.
    Boot {
        anchors.fill: parent
    }
}
