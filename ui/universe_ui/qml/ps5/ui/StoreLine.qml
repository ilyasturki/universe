import QtQuick
import "../core"

// A row of the Store's Downloads: the game's picture, its title over its state, the download's bar, the size, the action.
Item {
    id: line

    // A sources row, or null for a line of its own (Update everything).
    property var entry: null
    property string title: entry ? entry.title : ""
    property string meta: ""
    property string size: entry ? entry.sizeText : ""
    property string action: ""
    property string glyph: ""
    property bool focused: false
    property bool live: false
    property bool loud: false
    property real fraction: 0
    property bool showsBar: entry !== null && (entry.busy || entry.partial)

    signal picked

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: Theme.dp(4)
        anchors.bottomMargin: Theme.dp(4)
        radius: Theme.dp(Theme.radiusRow)
        color: line.focused ? Qt.rgba(1, 1, 1, 0.08) : "transparent"
        border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
        border.color: Theme.ringSoft

        Behavior on color {
            ColorAnimation {
                duration: Theme.durFocus
            }
        }
    }

    Rectangle {
        x: Theme.dp(136)
        anchors.bottom: parent.bottom
        width: parent.width - x - Theme.dp(20)
        height: 1
        color: Theme.hairline
        visible: !line.focused
    }

    StoreArt {
        id: thumb
        x: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(92)
        height: width
        visible: line.entry !== null
        entry: line.entry
        radius: Theme.dp(4)
        titleSize: Theme.dp(14)
    }

    Rectangle {
        x: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(92)
        height: width
        radius: Theme.dp(8)
        visible: line.entry === null
        color: Qt.rgba(1, 1, 1, 0.08)

        Glyph {
            anchors.centerIn: parent
            width: Theme.dp(40)
            height: width
            kind: line.glyph
        }
    }

    Column {
        x: Theme.dp(136)
        anchors.verticalCenter: parent.verticalCenter
        width: sizeText.x - x - Theme.dp(28)
        spacing: Theme.dp(4)

        Label {
            width: parent.width
            text: line.title
            elide: Text.ElideRight
        }

        Label {
            width: parent.width
            visible: text !== ""
            text: line.meta
            color: line.loud ? "#9cc8ff" : Theme.textSecondary
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }

        Item {
            width: parent.width
            height: Theme.dp(14)
            visible: line.showsBar

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                height: Theme.dp(5)
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.16)

                Rectangle {
                    width: parent.width * Math.max(0, Math.min(1, line.fraction))
                    height: parent.height
                    radius: parent.radius
                    color: line.live ? Theme.accent : Theme.textMuted

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.durQuick
                        }
                    }
                }
            }
        }
    }

    Label {
        id: sizeText
        anchors.right: actionText.left
        anchors.rightMargin: Theme.dp(28)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(170)
        horizontalAlignment: Text.AlignRight
        text: line.size
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
        font.features: {
            "tnum": 1
        }
    }

    Label {
        id: actionText
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(24)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(200)
        horizontalAlignment: Text.AlignRight
        text: line.action
        color: line.entry && line.entry.busy ? Theme.danger : line.focused ? Theme.text : Theme.textSecondary
        font.weight: line.focused ? Font.DemiBold : Font.Normal
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Touch {
        current: line.focused
        onPicked: line.picked()
    }
}
