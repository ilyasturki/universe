import QtQuick
import "../core"

// Where a slot stands, as a small pill: picked (yours), fetched, guessed (on disk, unrecorded), missing.
Rectangle {
    id: root

    property string kind: "missing"
    property string label: ""
    // On a lit (white) row the pill inverts.
    property bool onLight: false

    readonly property color tint: kind === "picked" ? "#5fd48a"
                                : kind === "fetched" ? "#7fb2ff"
                                : kind === "guessed" ? "#e0b45a"
                                : "#e0655a"

    width: body.width + Theme.dp(22)
    height: Theme.dp(28)
    radius: height / 2
    color: root.onLight ? Qt.rgba(0, 0, 0, 0.10) : Qt.rgba(tint.r, tint.g, tint.b, 0.18)
    border.width: 1
    border.color: root.onLight ? Qt.rgba(0, 0, 0, 0.18) : Qt.rgba(tint.r, tint.g, tint.b, 0.45)

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
            color: root.onLight ? Theme.onLight : Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(16)
        }
    }
}
