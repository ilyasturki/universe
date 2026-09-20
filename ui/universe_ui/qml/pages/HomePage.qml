import QtQuick
import Universe
import "../core"
import "../sound"
import "../core/Format.js" as Format
import "../ui"

FocusScope {
    id: page

    focus: true

    signal detailRequested(var game)
    signal tabRequested(int index)
    signal chromeRequested
    signal addRequested

    readonly property int railFloor: 5
    readonly property bool standingIn: (recent ? recent.count : 0) < railFloor
    readonly property var railModel: standingIn ? newest : recent
    readonly property int railCount: railModel ? railModel.count : 0

    readonly property var currentGame: anchor.game
    readonly property bool ownsBackdrop: true
    readonly property real backdropBlur: 0
    readonly property real scrimTop: 0
    readonly property real scrimMid: 0
    readonly property real scrimBottom: 0
    readonly property Item menuAnchor: tileSelected || !rail.currentItem ? null : rail.currentItem.artItem
    property bool menuOpen: false

    property bool tileSelected: false
    readonly property int focusIndex: tileSelected ? railCount : rail.currentIndex

    readonly property var hints: {
        var out = [];
        if (tileSelected) {
            out.push({
                glyph: "A",
                label: page.empty ? "Add a game" : "Open library"
            });
            out.push({
                glyph: "X",
                label: "Details",
                dim: true
            });
            out.push({
                glyph: "Y",
                label: favouriteLabel,
                dim: true
            });
        } else {
            out.push({
                glyph: "A",
                label: heroActions.activeFocus && heroActions.index === 1 ? "Details" : playLabel
            });
            out.push({
                glyph: "X",
                label: "Details"
            });
            out.push({
                glyph: "Y",
                label: favouriteLabel
            });
        }
        if (heroActions.activeFocus)
            out.push({
                glyph: "B",
                label: "Back to games"
            });
        return out;
    }

    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""
    readonly property string playLabel: currentGame && currentGame.id === playingId ? "Resume" : currentGame && currentGame.playTime > 0 ? "Continue" : "Play"
    readonly property string favouriteLabel: currentGame && currentGame.favorite ? "Remove from favourites" : "Add to favourites"

    readonly property real bandHeight: Theme.dp(Theme.heroBand)
    readonly property real railGapTop: Theme.dp(28)
    // The slot is the idle size: the focused card overhangs it and its neighbours step aside.
    readonly property real cellSize: Theme.dp(240)
    readonly property real slotSize: Theme.dp(176)
    readonly property real railGap: Theme.dp(24)
    readonly property real spread: (cellSize - slotSize) / 2
    readonly property real idleScale: 176 / 240

    readonly property int libraryCount: api.allGames.count
    readonly property int librarySeconds: api.allGames.totalPlayTime
    readonly property bool empty: libraryCount === 0

    onRailCountChanged: if (railCount === 0)
        tileSelected = true
    Component.onCompleted: if (railCount === 0)
        tileSelected = true

    function toggleFavourite() {
        if (!currentGame || tileSelected) {
            Sound.edge();
            return;
        }
        currentGame.favorite = !currentGame.favorite;
        Sound.favourite(currentGame.favorite);
    }

    onTileSelectedChanged: {
        heroActions.index = Math.min(heroActions.index, heroActions.last);
        rail.slideToCurrent();
    }

    function playingIndex() {
        for (var i = 0; i < railCount; i++)
            if (railModel.get(i).id === playingId)
                return i;
        return -1;
    }

    function playingTileRect(target) {
        var i = playingIndex();
        var item = i < 0 ? null : rail.itemAtIndex(i);
        if (!item || tileSelected)
            return null;
        var art = item.artItem;
        var a = art.mapToItem(target, 0, 0);
        var b = art.mapToItem(target, art.width, art.height);
        return Qt.rect(a.x, a.y, b.x - a.x, b.y - a.y);
    }

    // Where the tile will sit once the rail has slid, not where it is now.
    function landOnPlaying(target) {
        var i = playingIndex();
        if (i < 0)
            return null;
        tileSelected = false;
        rail.currentIndex = i;
        var restX = Math.max(0, Math.min(i * rail.pitch - spread - rail.width * 0.25, Math.max(0, rail.contentWidth - rail.width)));
        var p = rail.parent.mapToItem(target, rail.x + i * rail.pitch - restX + (slotSize - cellSize) / 2, rail.y);
        return Qt.rect(p.x, p.y, cellSize, cellSize);
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            toggleFavourite();
        } else if (page.tileSelected && api.keys.isAccept(event)) {
            event.accepted = true;
            page.empty ? page.addRequested() : page.tabRequested(1);
        } else if (page.tileSelected && api.keys.isDetails(event)) {
            event.accepted = true;
            Sound.edge();
        }
    }

    RecentGames {
        id: recent
        sourceModel: api.allGames
        playingId: page.playingId
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: page.railModel
        index: page.tileSelected ? -1 : rail.currentIndex
        onMoved: function (index) {
            page.tileSelected = false;
            rail.currentIndex = index;
        }
    }

    // The library exposes no date added; releaseYear is the closest "what is new".
    SortedGames {
        id: byRelease
        sourceModel: page.standingIn ? api.allGames : null
        sortRoleName: "releaseYear"
        descending: true
    }

    // The limit reads the source row, so the sort has to be its own proxy.
    LimitedGames {
        id: newest
        sourceModel: byRelease
        limit: 12
    }

    Repeater {
        id: logoPrefetch
        model: 5

        Image {
            // By residue, so a slot keeps its logo across a step.
            readonly property int base: rail.currentIndex - 2
            readonly property int at: base + (((index - base) % 5) + 5) % 5
            source: at >= 0 && at < page.railCount ? page.railModel.get(at).assets.logo : ""
            asynchronous: true
            visible: false
            // Must match HeroLogo's: sourceSize is part of the cache key.
            sourceSize.width: Theme.dp(480)
            sourceSize.height: Theme.dp(150)
        }
    }

    Item {
        id: band

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: page.bandHeight

        Item {
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(90)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(56)
            width: Theme.dp(900)
            height: heroActions.y + heroActions.height

            opacity: page.currentGame || page.tileSelected ? 1.0 : 0.0

            Behavior on opacity {
                Ease {
                    duration: Theme.durScene
                }
            }

            HeroLogo {
                id: heroLogo
                game: page.currentGame
                titleWidth: parent.width
                opacity: page.tileSelected ? 0.0 : 1.0

                Behavior on opacity {
                    Ease {}
                }
            }

            Text {
                anchors.bottom: heroLogo.bottom
                text: page.empty ? "Add your first game" : "Library"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(58)
                opacity: page.tileSelected ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            GameMetaLine {
                id: heroMeta
                anchors.top: heroLogo.bottom
                anchors.topMargin: Theme.dp(28)
                game: page.currentGame
                showYear: false
                opacity: page.tileSelected ? 0.0 : 1.0

                Behavior on opacity {
                    Ease {}
                }
            }

            Text {
                anchors.verticalCenter: heroMeta.verticalCenter
                text: page.empty ? "Nothing in the library yet: a file on this machine, a store, or your Lutris games." : Format.plural(page.libraryCount, "game", "games") + " · " + Format.totalPlayTime(page.librarySeconds) + " played"
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
                opacity: page.tileSelected ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            FocusScope {
                id: heroActions

                anchors.top: heroMeta.bottom
                anchors.topMargin: Theme.dp(34)
                width: buttons.width
                height: buttons.height

                property int index: 0
                readonly property int last: page.tileSelected ? 0 : 1

                function step(d) {
                    index = Sound.stepped(index, d, last + 1);
                }

                Row {
                    id: buttons
                    spacing: Theme.dp(36)
                    opacity: page.tileSelected ? 0.0 : 1.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        Ease {}
                    }

                    PillButton {
                        label: page.playLabel
                        focused: heroActions.activeFocus && heroActions.index === 0
                        dimmed: heroActions.activeFocus && heroActions.index !== 0
                    }

                    PillButton {
                        ghost: true
                        icon: "info"
                        label: "Details"
                        focused: heroActions.activeFocus && heroActions.index === 1
                        dimmed: heroActions.activeFocus && heroActions.index !== 1
                    }
                }

                PillButton {
                    icon: page.empty ? "plus" : "library"
                    label: page.empty ? "Add a game" : "Open library"
                    focused: heroActions.activeFocus && page.tileSelected
                    opacity: page.tileSelected ? 1.0 : 0.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        Ease {}
                    }
                }

                Keys.onLeftPressed: heroActions.step(-1)
                Keys.onRightPressed: heroActions.step(1)
                Keys.onUpPressed: page.chromeRequested()
                Keys.onDownPressed: function (event) {
                    Sound.panel();
                    rail.forceActiveFocus();
                }

                Keys.onPressed: function (event) {
                    if (api.keys.isAccept(event) && !page.tileSelected && heroActions.index === 1) {
                        event.accepted = true;
                        page.detailRequested(page.currentGame);
                    } else if (api.keys.isCancel(event)) {
                        event.accepted = true;
                        Sound.cancel();
                        rail.forceActiveFocus();
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.top: band.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.ground

        CapsLabel {
            id: standInNote

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(90)
            size: Theme.dp(15)
            visible: page.standingIn && !page.empty
            text: "FROM YOUR LIBRARY · RECENTLY PLAYED GOES HERE"
        }

        ListView {
            id: rail

            anchors.top: parent.top
            anchors.topMargin: page.railGapTop + (standInNote.visible ? standInNote.height + Theme.dp(14) : 0)
            anchors.left: parent.left
            anchors.right: parent.right
            // A header would move originX negative, and the view then refuses to scroll all the way to it.
            anchors.leftMargin: Theme.dp(90) + page.spread
            anchors.rightMargin: Theme.dp(90) - page.spread
            height: page.cellSize

            focus: true
            orientation: ListView.Horizontal
            model: page.railModel
            spacing: page.railGap
            interactive: false
            // interactive:false would otherwise take arrow-key navigation with it.
            keyNavigationEnabled: true
            // The default overshoot fixup fights the contentX Behavior and settles short of originX.
            boundsBehavior: Flickable.StopAtBounds
            highlightFollowsCurrentItem: false
            cacheBuffer: page.cellSize * 3

            readonly property real pitch: page.slotSize + page.railGap

            // A footer extends contentWidth without disturbing originX.
            footer: Item {
                width: page.railGap + page.slotSize + page.spread * 2
                height: page.cellSize

                LibraryTile {
                    width: page.cellSize
                    height: page.cellSize
                    anchors.bottom: parent.bottom
                    x: page.railGap + (page.slotSize - width) / 2 + (page.tileSelected ? 0 : page.spread)
                    kind: page.empty ? "add" : "library"
                    selected: page.tileSelected
                    idleScale: page.idleScale
                    count: page.libraryCount
                    ringOpacity: rail.activeFocus || page.menuOpen ? 1.0 : Theme.ringIdle

                    Behavior on x {
                        Ease {
                            easing.type: Easing.OutQuint
                        }
                    }
                }
            }

            function slideToCurrent() {
                if (width <= 0)
                    return;
                // page.focusIndex still holds the old index inside onCurrentIndexChanged.
                var index = page.tileSelected ? count : currentIndex;
                var target = index * pitch - page.spread - width * 0.25;
                var maxX = Math.max(0, contentWidth - width);
                contentX = Math.max(0, Math.min(target, maxX));
            }

            onCurrentIndexChanged: slideToCurrent()
            onWidthChanged: slideToCurrent()
            onCountChanged: slideToCurrent()

            Keys.onLeftPressed: function (event) {
                if (page.tileSelected) {
                    if (rail.count === 0) {
                        Sound.edge();
                        return;
                    }
                    page.tileSelected = false;
                    Sound.tick();
                    return;
                }
                event.accepted = false;
                rail.currentIndex > 0 ? Sound.tick() : Sound.edge();
            }
            Keys.onRightPressed: function (event) {
                if (page.tileSelected) {
                    Sound.edge();
                    return;
                }
                if (rail.currentIndex >= rail.count - 1) {
                    page.tileSelected = true;
                    Sound.tick();
                    return;
                }
                event.accepted = false;
                Sound.tick();
            }
            Keys.onUpPressed: function (event) {
                Sound.panel();
                heroActions.forceActiveFocus();
            }
            Keys.onDownPressed: Sound.edge()

            Behavior on contentX {
                Ease {
                    duration: Theme.durNudge
                    easing.type: Easing.OutQuint
                }
            }

            delegate: Item {
                id: tile

                readonly property bool selected: ListView.isCurrentItem && !page.tileSelected
                readonly property int delta: index - page.focusIndex
                property alias artItem: tileArt

                width: page.slotSize
                height: page.cellSize

                CoverCard {
                    id: tileArt

                    width: page.cellSize
                    height: page.cellSize
                    anchors.bottom: parent.bottom
                    x: (parent.width - width) / 2 + (tile.delta === 0 ? 0 : (tile.delta < 0 ? -page.spread : page.spread))

                    game: model
                    artSource: model.id === page.playingId && api.home.frame !== "" ? api.home.frame : String(model.assets.square) !== "" ? model.assets.square : model.assets.boxFront
                    playing: model.id === page.playingId
                    selected: tile.selected
                    selectedScale: 1.0
                    idleScale: page.idleScale
                    focusOrigin: Item.Bottom
                    cornerRadius: Theme.dp(Theme.radiusTile)
                    ringOpacity: rail.activeFocus || page.menuOpen ? 1.0 : Theme.ringIdle

                    Behavior on x {
                        Ease {
                            easing.type: Easing.OutQuint
                        }
                    }
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.top: rail.top
            anchors.bottom: rail.bottom
            // Stops short of where the focused ring's halo sits at either end.
            width: Theme.dp(90 - 14)
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop {
                    position: 0.0
                    color: Theme.ground
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0.055, 0.059, 0.075, 0.0)
                }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.top: rail.top
            anchors.bottom: rail.bottom
            width: Theme.dp(90 - 14)
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0.055, 0.059, 0.075, 0.0)
                }
                GradientStop {
                    position: 1.0
                    color: Theme.ground
                }
            }
        }
    }
}
