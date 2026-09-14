import QtQuick
import "../core"

// One image of the artwork page, framed, with its badge on the art and a caption under it.
Item {
    id: root

    property string source: ""
    property bool logo: false
    property string caption: ""
    property string kind: "missing"
    property string kindLabel: ""
    property bool dim: false

    readonly property real captionHeight: Theme.dp(30)

    RoundedMask {
        id: frame
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: root.captionHeight
        radius: Theme.dp(14)
        opacity: root.dim ? 0.7 : 1.0

        Rectangle {
            anchors.fill: parent
            color: root.logo ? Qt.rgba(1, 1, 1, 0.05) : Theme.surface
        }

        Image {
            anchors.fill: parent
            source: root.source
            fillMode: root.logo ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            asynchronous: true
            cache: false
            sourceSize.width: 1200
        }

        Text {
            anchors.centerIn: parent
            visible: root.source === ""
            text: "—"
            color: Theme.textFaint
            font.family: Theme.sans
            font.pixelSize: Theme.dp(40)
        }

        KindBadge {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.margins: Theme.dp(12)
            visible: root.kindLabel !== ""
            kind: root.kind
            label: root.kindLabel
        }
    }

    Text {
        anchors.top: frame.bottom
        anchors.topMargin: Theme.dp(8)
        anchors.left: parent.left
        anchors.right: parent.right
        text: root.caption
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(18)
        elide: Text.ElideRight
    }
}
