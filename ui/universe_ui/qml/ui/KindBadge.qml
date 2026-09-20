import QtQuick
import "../core"

Rectangle {
    id: root

    property string kind: "missing"
    property string label: ""
    property bool onLight: false

    readonly property color tint: kind === "picked" ? "#5fd48a" : kind === "default" ? "#7fb2ff" : "#e0655a"

    width: body.width + Theme.dp(22)
    height: Theme.dp(28)
    radius: height / 2
    color: root.onLight ? Qt.rgba(0, 0, 0, 0.10) : Qt.rgba(0, 0, 0, 0.62)
    border.width: 1
    border.color: root.onLight ? Qt.rgba(0, 0, 0, 0.18) : Qt.rgba(1, 1, 1, 0.14)

    Row {
        id: body
        anchors.centerIn: parent
        spacing: Theme.dp(7)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(9)
            height: width
            radius: width / 2
            color: root.tint
        }

        Text {
            id: text
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.onLight ? Theme.onLight : "#f2f3f5"
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(16)
        }
    }
}
