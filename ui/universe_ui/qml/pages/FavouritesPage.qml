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

    readonly property var currentGame: anchor ? anchor.game : null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 30
    readonly property real scrimTop: 0.72
    readonly property real scrimMid: 0.82
    readonly property real scrimBottom: 0.96
    readonly property Item menuAnchor: row.currentItem ? row.currentItem.artItem : null

    readonly property var hints: [
        {
            glyph: "A",
            label: "Launch"
        },
        {
            glyph: "X",
            label: "Details"
        },
        {
            glyph: "Y",
            label: currentGame && !currentGame.favorite ? "Add to favourites" : "Remove from favourites"
        }
    ]

    readonly property real cellWidth: Theme.dp(336)
    readonly property real cellHeight: Theme.dp(504)
    readonly property real idleScale: 304 / 336

    // Source rows unfavourited here: they stay, hollow, until the page is left, so Y can undo.
    property var pinned: []

    function toggleFavourite() {
        var g = currentGame;
        if (!g)
            return;
        if (g.favorite)
            pinned = pinned.concat([favourites.sourceRow(row.currentIndex)]);
        g.favorite = !g.favorite;
        Sound.favourite(g.favorite);
    }

    function leave() {
        if (pinned.length > 0)
            pinned = [];
    }

    FavouriteGames {
        id: favourites
        sourceModel: api.allGames
        pinned: page.pinned
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: favourites
        index: row.currentIndex
        onMoved: function (index) {
            row.currentIndex = index;
        }
    }

    Text {
        id: headTitle
        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(80)
        text: "Favourites"
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(46)
    }

    Text {
        anchors.centerIn: parent
        visible: !favourites || favourites.count === 0
        text: "No favourites yet — press Y on a game to add one."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(29)
    }

    ListView {
        id: row

        anchors.top: headTitle.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right

        focus: true
        visible: favourites && favourites.count > 0
        orientation: ListView.Horizontal
        model: favourites
        spacing: Theme.dp(40)
        interactive: false
        // interactive:false would otherwise take arrow-key navigation with it.
        keyNavigationEnabled: true
        cacheBuffer: page.cellWidth * 2
        highlightRangeMode: ListView.ApplyRange
        preferredHighlightBegin: (width - page.cellWidth) / 2
        preferredHighlightEnd: preferredHighlightBegin + page.cellWidth
        highlightMoveDuration: Theme.durView

        leftMargin: Math.max(Theme.dp(80), favourites ? (width - (favourites.count * cellWidth + Math.max(0, favourites.count - 1) * spacing)) / 2 : 0)

        delegate: Item {
            id: card

            readonly property bool selected: ListView.isCurrentItem
            readonly property bool hollow: !model.favorite
            property alias artItem: art

            width: page.cellWidth
            height: row.height

            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(18)

                CoverCard {
                    id: art
                    width: page.cellWidth
                    height: page.cellHeight
                    game: model
                    selected: card.selected
                    selectedScale: 1.0
                    idleScale: page.idleScale
                    cornerRadius: Theme.dp(14)
                    showHeart: false
                    pointable: true
                    current: card.selected && row.activeFocus
                    onPicked: row.currentIndex = index

                    Item {
                        anchors.fill: parent
                        transformOrigin: Item.Center
                        scale: card.selected ? 1.0 : page.idleScale
                        opacity: card.hollow ? 1.0 : 0.0
                        visible: opacity > 0.01

                        Behavior on scale {
                            Ease {
                                easing.type: Easing.OutQuint
                            }
                        }
                        Behavior on opacity {
                            Ease {}
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: Theme.dp(14)
                            color: Qt.rgba(0.055, 0.059, 0.075, 0.62)
                        }

                        MenuGlyph {
                            anchors.top: parent.top
                            anchors.right: parent.right
                            anchors.margins: Theme.dp(16)
                            width: Theme.dp(30)
                            height: Theme.dp(30)
                            kind: "heart-outline"
                        }
                    }
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: page.cellWidth
                    horizontalAlignment: Text.AlignHCenter
                    text: model.title
                    color: card.hollow ? Theme.textMuted : card.selected ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: card.selected ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(card.selected ? 27 : 24)
                    elide: Text.ElideRight
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: card.hollow ? "Removed · Y puts it back" : Format.playTime(model.playTime) + (card.selected && model.playCount > 0 ? " · " + Format.sessions(model.playCount) : "")
                    color: card.selected ? Theme.textSecondary : Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(21)
                }
            }
        }

        Keys.onLeftPressed: function (event) {
            event.accepted = false;
            row.currentIndex > 0 ? Sound.tick() : Sound.edge();
        }
        Keys.onRightPressed: function (event) {
            event.accepted = false;
            row.currentIndex < row.count - 1 ? Sound.tick() : Sound.edge();
        }
        Keys.onUpPressed: page.chromeRequested()
        Keys.onDownPressed: Sound.edge()

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isFilters(event)) {
                event.accepted = true;
                page.toggleFavourite();
            } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
                event.accepted = true;
                row.currentIndex = Sound.stepped(row.currentIndex, api.keys.isFirst(event) ? -row.count : row.count, row.count);
            }
        }

        Wheel {
            horizontal: true
            step: page.cellWidth + row.spacing
        }
    }
}
