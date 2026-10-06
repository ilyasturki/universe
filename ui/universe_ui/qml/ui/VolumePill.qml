import QtQuick

Item {
    id: root

    property real unit: 1
    readonly property bool silent: api.home.muted || api.home.volumePercent === 0

    Rectangle {
        id: osd
        objectName: "osdCard"

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 120 * root.unit
        width: 380 * root.unit
        height: 72 * root.unit
        radius: height / 2
        color: "#e0141418"
        opacity: api.home.osd && !api.home.open ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: 160
            }
        }

        MenuGlyph {
            id: speaker
            objectName: "osdGlyph"

            anchors.left: parent.left
            anchors.leftMargin: 26 * root.unit
            anchors.verticalCenter: parent.verticalCenter
            width: 30 * root.unit
            height: width
            kind: root.silent ? "mute" : "volume-up"
        }

        Text {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * root.unit
            anchors.right: percent.left
            anchors.rightMargin: 12 * root.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * root.unit
            text: api.home.volumeOutput
            color: "#ffffff"
            elide: Text.ElideRight
            font.pixelSize: 16 * root.unit
        }

        Text {
            id: percent
            objectName: "osdPercent"

            anchors.right: parent.right
            anchors.rightMargin: 28 * root.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * root.unit
            text: api.home.volumePercent + "%"
            color: "#b8ffffff"
            opacity: api.home.muted ? 0.45 : 1
            font.pixelSize: 16 * root.unit
        }

        Rectangle {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * root.unit
            anchors.right: parent.right
            anchors.rightMargin: 28 * root.unit
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 18 * root.unit
            height: 6 * root.unit
            radius: height / 2
            color: "#40ffffff"

            Rectangle {
                objectName: "osdFill"

                width: parent.width * Math.min(api.home.volumePercent, 100) / 100
                height: parent.height
                radius: parent.radius
                color: "#ffffff"
                opacity: api.home.muted ? 0.4 : 1

                Behavior on width {
                    NumberAnimation {
                        duration: 120
                    }
                }
            }
        }
    }
}
