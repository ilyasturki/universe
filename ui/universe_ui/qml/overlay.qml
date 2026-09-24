import QtQuick
import QtQuick.Window
import "ui"

Window {
    id: overlay

    readonly property real unit: Math.max(1, height / 1080)

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
        id: osd

        readonly property bool silent: api.home.muted || api.home.volumePercent === 0

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 120 * overlay.unit
        width: 380 * overlay.unit
        height: 72 * overlay.unit
        radius: height / 2
        color: "#e0141418"
        opacity: api.home.osd && !api.home.open ? 1 : 0
        z: 5

        Behavior on opacity {
            NumberAnimation {
                duration: 160
            }
        }

        MenuGlyph {
            id: speaker

            anchors.left: parent.left
            anchors.leftMargin: 26 * overlay.unit
            anchors.verticalCenter: parent.verticalCenter
            width: 30 * overlay.unit
            height: width
            kind: osd.silent ? "mute" : "volume-up"
        }

        Text {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * overlay.unit
            anchors.right: percent.left
            anchors.rightMargin: 12 * overlay.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * overlay.unit
            text: api.home.volumeOutput
            color: "#ffffff"
            elide: Text.ElideRight
            font.pixelSize: 16 * overlay.unit
        }

        Text {
            id: percent

            anchors.right: parent.right
            anchors.rightMargin: 28 * overlay.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * overlay.unit
            text: osd.silent ? "Muted" : api.home.volumePercent + "%"
            color: "#b8ffffff"
            font.pixelSize: 16 * overlay.unit
        }

        Rectangle {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * overlay.unit
            anchors.right: parent.right
            anchors.rightMargin: 28 * overlay.unit
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 18 * overlay.unit
            height: 6 * overlay.unit
            radius: height / 2
            color: "#40ffffff"

            Rectangle {
                width: parent.width * (osd.silent ? 0 : Math.min(api.home.volumePercent, 100) / 100)
                height: parent.height
                radius: parent.radius
                color: "#ffffff"

                Behavior on width {
                    NumberAnimation {
                        duration: 120
                    }
                }
            }
        }
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
