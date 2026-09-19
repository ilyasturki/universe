import QtQuick
import "../core"

// A slot row's art at the slot's shape: a logo fitted, the rest cropped, an empty slot said so.
RoundedMask {
    id: root

    property var row: null
    property bool badge: false
    property bool dim: false
    property string emptyText: "Nothing yet"
    property real emptySize: Theme.dp(20)
    property real badgeMargin: Theme.dp(12)
    property string badgeLabel: ""
    readonly property bool empty: row === null || !row.url
    readonly property bool logo: row !== null && row.slot === "logo"

    radius: Theme.dp(12)
    opacity: dim ? 0.6 : 1.0

    Rectangle {
        anchors.fill: parent
        radius: root.radius
        color: root.empty ? Qt.rgba(0.88, 0.40, 0.35, 0.08) : root.logo ? Qt.rgba(1, 1, 1, 0.05) : Theme.surface
        border.width: root.empty ? 2 : 0
        border.color: Qt.rgba(0.88, 0.40, 0.35, 0.5)
    }

    Image {
        anchors.fill: parent
        source: root.row && root.row.url ? root.row.url : ""
        fillMode: root.logo ? Image.PreserveAspectFit : Image.PreserveAspectCrop
        asynchronous: true
        sourceSize.width: 1200
    }

    Text {
        anchors.centerIn: parent
        visible: root.empty
        text: root.emptyText
        color: "#e0655a"
        font.family: Theme.sans
        font.weight: Font.DemiBold
        font.pixelSize: root.emptySize
    }

    KindBadge {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.margins: root.badgeMargin
        visible: root.badge && root.row !== null && (root.badgeLabel !== "" || !!root.row.kindLabel)
        kind: root.row ? root.row.kind : "missing"
        label: root.badgeLabel !== "" ? root.badgeLabel : root.row && root.row.kindLabel ? root.row.kindLabel : ""
    }
}
