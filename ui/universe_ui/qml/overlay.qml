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

    Rectangle {
        id: flash

        anchors.fill: parent
        color: "white"
        opacity: 0.0
        z: 10

        SequentialAnimation {
            id: flashAnim
            NumberAnimation {
                target: flash
                property: "opacity"
                to: 0.85
                duration: 40
            }
            NumberAnimation {
                target: flash
                property: "opacity"
                to: 0.0
                duration: 320
                easing.type: Easing.OutQuad
            }
        }

        Connections {
            target: api.home
            function onScreenshotTaken(path) {
                if (path)
                    flashAnim.restart();
            }
        }
    }
}
