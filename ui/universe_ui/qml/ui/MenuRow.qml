import QtQuick
import "../core"

// Item: { icon, image, label, detail, more, danger, active }; `image` is a file under qml/ or a file: URL drawn where the glyph goes,
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
    readonly property bool hasDetail: item.detail !== undefined && item.detail !== ""
    readonly property bool hasMark: item.active === true || item.more === true
    readonly property real wordsX: hasImage ? Theme.dp(18 + 34 + 18) : hasGlyph ? Theme.dp(22 + 26 + 18) : Theme.dp(22)
    readonly property real beside: width - wordsX - Theme.dp(12 + 20) - (hasMark ? Theme.dp(22) : 0) - (hasDetail ? detailWidth.advanceWidth + (hasMark ? Theme.dp(8) : 0) : 0)
    // The label wins: a detail that would cut it goes under it.
    readonly property bool detailUnder: hasDetail && labelWidth.advanceWidth > beside

    height: Math.max(Theme.dp(66), words.implicitHeight + Theme.dp(24))
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
        source: !row.hasImage ? "" : row.item.image.indexOf("file:") === 0 ? row.item.image : Qt.resolvedUrl("../" + row.item.image)
        asynchronous: true
        fillMode: Image.PreserveAspectFit
        sourceSize.height: 128
        smooth: true
        mipmap: true
    }

    TextMetrics {
        id: labelWidth
        text: row.item.label || ""
        font.family: Theme.sans
        font.weight: Font.DemiBold
        font.pixelSize: Theme.dp(25)
    }

    TextMetrics {
        id: detailWidth
        text: row.item.detail || ""
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: Theme.dp(21)
    }

    Column {
        id: words

        x: row.wordsX
        width: trailing.x - x - Theme.dp(12)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(2)

        Text {
            width: parent.width
            text: row.item.label || ""
            color: row.ink
            font.family: Theme.sans
            font.weight: row.focused ? Font.DemiBold : Font.Medium
            font.pixelSize: Theme.dp(25)
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            visible: row.detailUnder
            text: row.item.detail || ""
            color: row.inkSoft
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(21)
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
        }
    }

    Row {
        id: trailing

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(8)

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: row.hasDetail && !row.detailUnder
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
            visible: row.hasMark
            kind: row.item.active === true ? "check" : "chevron"
            tint: row.item.active === true ? row.ink : row.inkSoft
        }
    }
}
