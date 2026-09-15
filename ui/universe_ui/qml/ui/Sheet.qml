import QtQuick
import "../core"

FocusScope {
    id: sheet

    property bool open: false
    property string title: ""
    property real innerMax: Theme.dp(1000)
    // The content's height under the heading; the panel wraps it.
    property real contentHeight: 0
    readonly property real inner: Math.min(innerMax, width - Theme.dp(280))
    readonly property real pad: Theme.dp(28)
    readonly property Item head: heading
    default property alias content: panel.data

    visible: scrim.opacity > 0.01
    focus: open

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: sheet.open ? 0.72 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Item {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        height: sheet.pad * 2 + heading.height + sheet.contentHeight
        y: sheet.open ? parent.height - height - Theme.dp(Theme.hintBarHeight) : parent.height

        Behavior on y {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: -Theme.dp(120)
            radius: Theme.dp(30)
            color: Qt.rgba(0.071, 0.075, 0.094, 1.0)
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        Text {
            id: heading
            anchors.top: parent.top
            anchors.topMargin: sheet.pad
            anchors.horizontalCenter: parent.horizontalCenter
            width: sheet.inner
            text: sheet.title
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
        }
    }
}
