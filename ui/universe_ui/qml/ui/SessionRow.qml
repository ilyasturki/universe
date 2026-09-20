import QtQuick
import "../core"

Rectangle {
    id: row

    property string title: ""
    property string subtitle: ""
    property bool lit: false
    property bool muted: false
    property string mark: ""
    property bool showMark: false
    property real leadWidth: 0
    property real leadMargin: Theme.dp(22)
    property real gap: Theme.dp(20)
    default property alias lead: leadSlot.data

    radius: Theme.dp(16)
    color: lit ? Theme.text : Theme.surface

    Behavior on color {
        ColorEase {}
    }

    Item {
        id: leadSlot
        anchors.left: parent.left
        anchors.leftMargin: row.leadMargin
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: row.leadWidth
    }

    Column {
        anchors.left: leadSlot.right
        anchors.leftMargin: row.leadWidth > 0 ? row.gap : 0
        anchors.right: markGlyph.visible ? markGlyph.left : parent.right
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(6)

        Text {
            width: parent.width
            text: row.title
            color: row.lit ? Theme.onLight : row.muted ? Theme.textSecondary : Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(24)
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            text: row.subtitle
            color: row.lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
            elide: Text.ElideRight
        }
    }

    MenuGlyph {
        id: markGlyph
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(22)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(24)
        height: width
        visible: row.showMark
        kind: row.mark
        tint: row.lit ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
    }
}
