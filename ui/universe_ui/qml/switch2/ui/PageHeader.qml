import QtQuick
import "../core"

Item {
    id: header

    property string icon: ""
    property var game: null
    property string title: ""
    property string subtitle: ""
    property string trailing: ""
    property color iconColor: Theme.text
    property bool hairline: true

    implicitHeight: Theme.dp(Theme.headerHeight)

    Glyph {
        id: glyph
        visible: header.icon !== ""
        x: Theme.dp(120)
        y: Theme.dp(50)
        width: Theme.dp(50)
        height: Theme.dp(50)
        kind: header.icon
        tint: header.iconColor
    }

    Tile {
        id: thumb
        visible: header.game !== null
        x: Theme.dp(120)
        y: Theme.dp(30)
        width: Theme.dp(84)
        height: Theme.dp(84)
        cornerRadius: Theme.dp(6)
        outlineShown: false
        game: header.game
    }

    Column {
        x: header.icon !== "" ? glyph.x + glyph.width + Theme.dp(18) : thumb.visible ? thumb.x + thumb.width + Theme.dp(24) : Theme.dp(120)
        anchors.verticalCenter: glyph.verticalCenter
        spacing: Theme.dp(2)

        Label {
            visible: header.subtitle !== ""
            text: header.subtitle
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Label {
            text: header.title
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }
    }

    Label {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(120)
        anchors.verticalCenter: glyph.verticalCenter
        text: header.trailing
        color: Theme.textSecondary
    }

    Hairline {
        anchors.leftMargin: Theme.dp(Theme.edgeMargin)
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        visible: header.hairline
        color: Theme.hairline
    }
}
