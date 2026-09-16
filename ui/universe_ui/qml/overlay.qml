import QtQuick
import QtQuick.Window

Window {
    id: overlay

    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: "Universe home"
    visible: false

    Loader {
        anchors.fill: parent
        source: api.theme.overlay
        focus: true
    }
}
