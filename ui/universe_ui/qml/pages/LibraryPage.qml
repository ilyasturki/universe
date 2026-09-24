import QtQuick
import Universe
import "../core"
import "../sound"
import "../core/Format.js" as Format
import "../ui"

FocusScope {
    id: page

    focus: true

    signal chromeRequested
    signal addRequested

    readonly property var currentGame: anchor ? anchor.game : null
    readonly property bool onAddTile: grid.addSelected
    readonly property bool empty: api.allGames.count === 0
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 18
    readonly property real scrimTop: 0.58
    readonly property real scrimMid: 0.74
    readonly property real scrimBottom: 0.95
    readonly property Item menuAnchor: grid.focusedArtItem
    property bool menuOpen: false

    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""

    readonly property var hints: onAddTile ? [
        {
            glyph: "A",
            label: "Add a game"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ] : [
        {
            glyph: "A",
            label: Format.playLabel(currentGame, playingId)
        },
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property int columns: 8
    readonly property real gap: Theme.dp(28)
    readonly property real sideMargin: Theme.dp(80)
    readonly property real gridMargin: sideMargin - gap / 2
    readonly property real cellWidth: (width - gridMargin * 2) / columns

    function toggleFavourite() {
        if (!currentGame) {
            Sound.edge();
            return;
        }
        currentGame.favorite = !currentGame.favorite;
        Sound.favourite(currentGame.favorite);
    }

    FavouritesFirstGames {
        id: shelf
        sourceModel: api.allGames
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: shelf
        index: grid.addSelected ? -1 : grid.currentIndex
        onMoved: function (index) {
            grid.addPicked = false;
            grid.currentIndex = index;
        }
    }

    Text {
        id: titleText

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        text: page.currentGame ? page.currentGame.title : page.onAddTile ? (page.empty ? "Add your first game" : "Add a game") : ""
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(58)
        elide: Text.ElideRight
    }

    CoverGrid {
        id: grid

        anchors.top: titleText.bottom
        anchors.topMargin: Theme.dp(34) - grid.inset
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.gridMargin - grid.inset
        anchors.rightMargin: page.gridMargin - grid.inset

        focus: true
        model: shelf
        columns: page.columns
        gap: page.gap
        cellWidth: page.cellWidth
        selectionActive: grid.activeFocus || page.menuOpen
        addTile: true

        onPointed: grid.forceActiveFocus()

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (grid.addSelected && api.keys.isAccept(event)) {
                event.accepted = true;
                page.addRequested();
            } else if (grid.addSelected && (api.keys.isDetails(event) || api.keys.isMenu(event))) {
                event.accepted = true;
                Sound.edge();
            } else if (api.keys.isFilters(event)) {
                event.accepted = true;
                page.toggleFavourite();
            }
        }
    }

    Keys.onUpPressed: page.chromeRequested()
}
