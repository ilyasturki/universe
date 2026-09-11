import QtQuick
import QtQuick.Window

Window {
    id: window

    width: 1920
    height: 1080
    visible: true
    visibility: api.fullscreen ? Window.FullScreen : Window.Windowed
    color: "#0e0f13"
    title: "Universe"

    Loader {
        anchors.fill: parent
        source: "theme.qml"
        focus: true
    }
}
