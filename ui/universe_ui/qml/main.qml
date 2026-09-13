import QtQuick
import QtQuick.Window

Window {
    id: window

    width: 1920
    height: 1080
    visibility: api.fullscreen ? Window.FullScreen : Window.Windowed
    color: api.theme.ground
    title: "Universe"

    Loader {
        anchors.fill: parent
        source: api.theme.entry
        focus: true
    }
}
