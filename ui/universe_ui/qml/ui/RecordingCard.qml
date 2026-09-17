import QtQuick
import "../core"

Column {
    id: card

    property var recording: null
    property bool focused: false
    property bool dimmed: false

    readonly property var frames: recording && api.screens.recordings.frameMap[recording.session] || null
    readonly property real shotWidth: Theme.dp(336)
    readonly property real shotHeight: Theme.dp(189)

    spacing: Theme.dp(18)
    visible: recording !== null

    CapsLabel {
        text: "RECORDING"
        tracking: 0.11
        color: card.focused ? Theme.textSecondary : Theme.textMuted
    }

    Item {
        width: parent.width
        height: card.shotHeight
        opacity: card.dimmed ? 0.6 : 1.0

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        RoundedMask {
            id: thumb

            width: card.shotWidth
            height: card.shotHeight
            radius: Theme.dp(10)
            scale: card.focused ? 1.03 : 1.0

            Behavior on scale { Ease { easing.type: Easing.OutQuint } }

            Rectangle {
                anchors.fill: parent
                color: Theme.cardBase
            }

            Image {
                anchors.fill: parent
                source: card.frames ? card.frames.thumbnail : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                opacity: status === Image.Ready ? 1.0 : 0.0

                Behavior on opacity { Ease { duration: Theme.durView } }
            }

            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0.02, 0.02, 0.03, 0.30)
            }

            Rectangle {
                anchors.centerIn: parent
                width: Theme.dp(64)
                height: width
                radius: width / 2
                color: Qt.rgba(1, 1, 1, 0.92)

                MenuGlyph {
                    anchors.centerIn: parent
                    anchors.horizontalCenterOffset: Theme.dp(2)
                    width: Theme.dp(32)
                    height: width
                    kind: "play"
                    tint: "#101116"
                }
            }
        }

        Loader {
            anchors.fill: thumb
            active: card.focused
            sourceComponent: FocusRing {
                cornerRadius: thumb.radius
                gapWidth: Theme.dp(4)
            }
        }

        Column {
            anchors.left: thumb.right
            anchors.leftMargin: Theme.dp(36)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(8)

            Text {
                width: parent.width
                text: card.recording ? card.recording.durationText + "  ·  " + card.recording.sizeText : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(26)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: card.recording ? card.recording.dateText : ""
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(22)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: card.recording ? card.recording.path.split("/").pop() : ""
                color: Theme.textFaint
                font.family: Theme.sans
                font.pixelSize: Theme.dp(20)
                elide: Text.ElideMiddle
            }
        }
    }
}
