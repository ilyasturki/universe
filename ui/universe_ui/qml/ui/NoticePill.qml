import QtQuick
import "../core"

Item {
    id: pill

    property real maxWidth: implicitWidth
    readonly property bool error: Notices.error

    implicitWidth: row.implicitWidth + Theme.dp(44)
    implicitHeight: Theme.dp(48)
    width: Math.min(implicitWidth, maxWidth)
    height: implicitHeight
    opacity: Notices.current !== null ? 1.0 : 0.0
    visible: opacity > 0.01

    Behavior on opacity {
        Ease {}
    }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: "#1b1d24"
        border.width: 1
        border.color: pill.error ? Qt.alpha(Theme.danger, 0.7) : Theme.surfaceBorder
    }

    Row {
        id: row

        x: Theme.dp(22)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(12)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(10)
            height: width
            radius: width / 2
            color: Theme.danger
            visible: pill.error
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, pill.maxWidth - Theme.dp(44) - (pill.error ? Theme.dp(22) : 0))
            text: Notices.text
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(20)
            elide: Text.ElideRight
        }
    }
}
