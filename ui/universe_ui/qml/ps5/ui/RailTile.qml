import QtQuick
import "../core"

// A tile of the home row: a game's art, or one of the console's own tiles (dark glass, a white icon).
Item {
    id: tile

    property var game: null
    // "game", else the console tile it is: welcome, store, gallery, library, journal.
    property string kind: "game"
    property bool focused: false

    readonly property real radius: Math.round(width * 0.1)
    readonly property bool installing: game !== null && game !== undefined && game.installing === true

    readonly property var icons: ({
            welcome: "welcome",
            store: "store",
            gallery: "gallery",
            library: "library",
            journal: "journal",
            add: "plus",
            setup: "settings"
        })

    TileArt {
        anchors.fill: parent
        visible: tile.kind === "game"
        game: tile.kind === "game" ? tile.game : null
        radius: tile.radius
        dimmed: tile.installing
        titleSize: Math.max(Theme.dp(12), tile.width / 8)
    }

    Rectangle {
        anchors.fill: parent
        visible: tile.kind !== "game"
        radius: tile.radius
        color: Qt.rgba(0.14, 0.15, 0.19, 0.86)
        border.width: 1
        border.color: Theme.glassEdge

        Glyph {
            anchors.centerIn: parent
            width: Math.round(parent.width * 0.5)
            height: width
            kind: tile.icons[tile.kind] || ""
            stroke: 1.6
        }
    }

    // A store install under way: the console's thin bar along the tile's foot.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Math.round(tile.width * 0.08)
        height: Math.max(3, Math.round(tile.width * 0.035))
        radius: height / 2
        visible: tile.installing
        color: Qt.rgba(1, 1, 1, 0.25)

        Rectangle {
            width: parent.width * Math.max(0.02, tile.game && tile.game.progress >= 0 ? tile.game.progress : 0)
            height: parent.height
            radius: parent.radius
            color: Theme.text
        }
    }

    FocusFrame {
        target: tile
        shown: tile.focused
        radius: tile.radius
        gap: Theme.dp(4)
        line: Theme.dp(3)
        color: Qt.rgba(1, 1, 1, 0.78)
    }
}
