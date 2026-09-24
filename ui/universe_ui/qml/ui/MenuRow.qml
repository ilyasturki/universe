import QtQuick
import "../core"

// Item: { icon, image, label, detail, more, danger, active }; `image` is a file under qml/ drawn where the glyph goes,
// `active` checks the value a setting has now.
Rectangle {
    id: row

    property var item: ({})
    property bool focused: false

    signal picked

    readonly property bool danger: item.danger === true
    readonly property color ink: focused ? Theme.onLight : danger ? "#e0655a" : Theme.text
    readonly property color inkSoft: focused ? Theme.onLight : Theme.textSecondary
    readonly property bool hasImage: item.image !== undefined && item.image !== ""
    readonly property bool hasGlyph: !hasImage && item.icon !== undefined && item.icon !== ""

    height: Theme.dp(66)
    radius: Theme.dp(16)
    color: focused ? (danger ? "#e0655a" : Theme.text) : "transparent"

    Behavior on color {
        ColorEase {}
    }

    Pointer {
        direct: true
        radius: Theme.dp(16)
        onPicked: row.picked()
    }

    MenuGlyph {
        id: glyph

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(22)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(26)
        height: Theme.dp(26)
        visible: row.hasGlyph
        kind: row.item.icon || ""
        tint: row.ink
    }

    Image {
        id: picture

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(18)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(34)
        height: width
        visible: row.hasImage
        source: row.hasImage ? Qt.resolvedUrl("../" + row.item.image) : ""
        asynchronous: true
        fillMode: Image.PreserveAspectFit
        sourceSize.height: 128
        smooth: true
        mipmap: true
    }

    Text {
        anchors.left: row.hasImage ? picture.right : glyph.visible ? glyph.right : parent.left
        anchors.leftMargin: row.hasImage || glyph.visible ? Theme.dp(18) : Theme.dp(22)
        anchors.right: trailing.left
        anchors.rightMargin: Theme.dp(12)
        anchors.verticalCenter: parent.verticalCenter
        text: row.item.label || ""
        color: row.ink
        font.family: Theme.sans
        font.weight: row.focused ? Font.DemiBold : Font.Medium
        font.pixelSize: Theme.dp(25)
        elide: Text.ElideRight
    }

    Row {
        id: trailing

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(8)

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: row.item.detail !== undefined && row.item.detail !== ""
            text: row.item.detail || ""
            color: row.inkSoft
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(21)
        }

        MenuGlyph {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(22)
            height: Theme.dp(22)
            visible: row.item.active === true || row.item.more === true
            kind: row.item.active === true ? "check" : "chevron"
            tint: row.item.active === true ? row.ink : row.inkSoft
        }
    }
}
