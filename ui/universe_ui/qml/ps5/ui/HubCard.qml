import QtQuick
import "../core"
import "../../ui" as Base

// A card of the Game Hub and the Control Center: dark glass or a picture, a small white badge top left,
// a muted line over a bold one at the foot; the focused one outlined.
Item {
    id: card

    property url image: ""
    property bool imageFill: true
    property string badge: ""
    property string caption: ""
    property string title: ""
    property string body: ""
    property real progress: -1
    property bool focused: false
    property bool playIcon: false
    property color tint: Theme.glass

    readonly property real radius: Theme.dp(Theme.radiusCard)
    readonly property bool pictured: String(image) !== "" && picture.status === Image.Ready

    signal picked

    Base.RoundedMask {
        anchors.fill: parent
        radius: card.radius

        Rectangle {
            anchors.fill: parent
            radius: Theme.software ? card.radius : 0
            color: card.tint
        }

        Image {
            id: picture
            anchors.fill: parent
            anchors.margins: card.imageFill ? 0 : Theme.dp(24)
            anchors.bottomMargin: card.imageFill ? 0 : Theme.dp(110)
            anchors.topMargin: card.imageFill ? 0 : Theme.dp(78)
            source: card.image
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            smooth: true
            sourceSize.width: 720
            visible: status === Image.Ready
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.height * 0.62
            visible: card.pictured && card.imageFill
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0.78)
                }
            }
        }

        Rectangle {
            anchors.centerIn: picture
            width: Theme.dp(64)
            height: width
            radius: width / 2
            visible: card.playIcon
            color: Qt.rgba(0, 0, 0, 0.5)
            border.width: Theme.dp(2)
            border.color: Qt.rgba(1, 1, 1, 0.8)

            Glyph {
                anchors.centerIn: parent
                anchors.horizontalCenterOffset: Theme.dp(2)
                width: Theme.dp(28)
                height: width
                kind: "play"
            }
        }
    }

    Rectangle {
        x: Theme.dp(16)
        y: Theme.dp(16)
        width: Theme.dp(46)
        height: width
        radius: Theme.dp(8)
        visible: card.badge !== ""
        color: "#ffffff"

        Glyph {
            anchors.centerIn: parent
            width: Theme.dp(30)
            height: width
            kind: card.badge
            tint: Theme.onLight
            stroke: 2
        }
    }

    Label {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Theme.dp(24)
        anchors.topMargin: card.badge !== "" ? Theme.dp(80) : Theme.dp(24)
        visible: card.body !== ""
        text: card.body
        color: Theme.textSecondary
        wrapMode: Text.WordWrap
        elide: Text.ElideRight
        maximumLineCount: Math.max(1, Math.floor((card.height - (card.badge !== "" ? Theme.dp(80) : Theme.dp(24)) - foot.height - Theme.dp(40)) / (font.pixelSize * 1.3)))
        lineHeight: 1.25
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Column {
        id: foot

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Theme.dp(24)
        spacing: Theme.dp(4)

        Label {
            width: parent.width
            visible: card.caption !== ""
            text: card.caption
            color: Theme.textSecondary
            elide: Text.ElideRight
            font.weight: Font.Light
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Label {
            width: parent.width
            visible: card.title !== ""
            text: card.title
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(29)
        }

        Rectangle {
            width: parent.width
            height: Theme.dp(6)
            radius: height / 2
            visible: card.progress >= 0
            color: Qt.rgba(1, 1, 1, 0.18)

            Rectangle {
                width: parent.width * Math.max(0, Math.min(1, card.progress))
                height: parent.height
                radius: parent.radius
                color: Theme.text
            }
        }
    }

    FocusFrame {
        target: card
        shown: card.focused
        radius: card.radius
        gap: Theme.dp(3)
        line: Theme.dp(3)
    }

    Touch {
        current: card.focused
        onPicked: card.picked()
    }
}
