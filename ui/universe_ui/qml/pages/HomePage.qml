import QtQuick
import Universe
import "../core"
import "../sound"
import "../core/Format.js" as Format
import "../ui"

FocusScope {
    id: page

    focus: true

    signal libraryRequested
    signal chromeRequested
    signal addRequested
    signal setupRequested

    readonly property int railFloor: 5
    readonly property bool standingIn: (recent ? recent.count : 0) < railFloor
    readonly property var railModel: shown
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
    readonly property bool onSetup: tileSelected && empty && heroActions.activeFocus && heroActions.index === 1

    readonly property var hints: {
        var out = [];
        if (tileSelected)
            out.push({
                glyph: "A",
                label: onSetup ? "Set up" : page.empty ? "Add a game" : "Open"
            });
        else
            out.push({
                glyph: "A",
                label: playLabel
            }, {
                glyph: "Start",
                label: "More",
                dim: arriving
            });
        if (heroActions.activeFocus)
            out.push({
                glyph: "B",
                label: "Back"
            });
        return out;
    }

    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""
    readonly property bool arriving: currentGame !== null && currentGame.installing
    readonly property string playLabel: Format.playLabel(currentGame, playingId)

    readonly property real bandHeight: Theme.dp(Theme.heroBand)
    readonly property real railGapTop: Theme.dp(28)
    // The slot is the idle size: the focused card overhangs it and its neighbours step aside.
    readonly property real cellSize: Theme.dp(240)
    readonly property real slotSize: Theme.dp(176)
    readonly property real railGap: Theme.dp(24)
    readonly property real spread: (cellSize - slotSize) / 2
    readonly property real idleScale: 176 / 240

    readonly property bool empty: api.allGames.count === 0

    onRailCountChanged: if (railCount === 0)
        tileSelected = true
    Component.onCompleted: if (railCount === 0)
        tileSelected = true

    function toggleFavourite() {
        if (!currentGame || tileSelected || arriving) {
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

    // The mouse lands the ring without the pad's tick: on a game, on the tile past them, or on a hero pill.
    function pointToTile(index) {
        rail.forceActiveFocus();
        if (index >= railCount) {
            tileSelected = true;
            return;
        }
        tileSelected = false;
        rail.currentIndex = index;
    }

    function pointToAction(index) {
        heroActions.index = index;
        heroActions.forceActiveFocus();
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            toggleFavourite();
        } else if (page.tileSelected && api.keys.isAccept(event)) {
            event.accepted = true;
            page.onSetup ? page.setupRequested() : page.empty ? page.addRequested() : page.libraryRequested();
        } else if (page.tileSelected && api.keys.isDetails(event)) {
            event.accepted = true;
            Sound.edge();
        }
    }

    RecentGames {
        id: played
        sourceModel: api.allGames
        playingId: page.playingId
    }

    // The rail holds the last twelve, as the Switch 2 look's does; the library tile has the rest.
    LimitedGames {
        id: recent
        sourceModel: played
        limit: 12
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

    // A library that predates `added_at` has nothing recent to show; releaseYear is the closest "what is new".
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

    // A store install under way sits first; its tile is the game's once it lands.
    HeadedGames {
        id: shown
        source: page.standingIn ? newest : recent
        head: api.screens.sources.arriving
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
                text: page.empty ? "Welcome" : "Library"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(58)
                opacity: page.tileSelected ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            Text {
                id: emptyNote
                anchors.top: heroLogo.bottom
                anchors.topMargin: Theme.dp(28)
                text: "Add a game from this machine, a store or Lutris."
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
                opacity: page.tileSelected && page.empty ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            FocusScope {
                id: heroActions

                anchors.top: page.empty ? emptyNote.bottom : heroLogo.bottom
                anchors.topMargin: Theme.dp(34)
                width: page.tileSelected ? tileButtons.width : buttons.width
                height: buttons.height

                property int index: 0
                readonly property int last: page.tileSelected && page.empty ? 1 : 0

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
                        focused: heroActions.activeFocus
                        onPicked: page.pointToAction(0)
                    }
                }

                Row {
                    id: tileButtons
                    spacing: Theme.dp(36)
                    opacity: page.tileSelected ? 1.0 : 0.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        Ease {}
                    }

                    PillButton {
                        icon: page.empty ? "plus" : "library"
                        label: page.empty ? "Add a game" : "Open"
                        focused: heroActions.activeFocus && heroActions.index === 0
                        dimmed: heroActions.activeFocus && heroActions.index !== 0
                        onPicked: page.pointToAction(0)
                    }

                    PillButton {
                        ghost: true
                        icon: ""
                        label: "Set up"
                        visible: page.empty
                        focused: heroActions.activeFocus && heroActions.index === 1
                        dimmed: heroActions.activeFocus && heroActions.index !== 1
                        onPicked: page.pointToAction(1)
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
                    if (api.keys.isCancel(event)) {
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

        ListView {
            id: rail

            anchors.top: parent.top
            anchors.topMargin: page.railGapTop
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
                    ringOpacity: rail.activeFocus || page.menuOpen ? 1.0 : Theme.ringIdle
                    pointable: true
                    current: page.tileSelected && rail.activeFocus
                    onPicked: page.pointToTile(page.railCount)

                    Behavior on x {
                        Ease {
                            easing.type: Easing.OutQuint
                        }
                    }
                }
            }

            Wheel {
                horizontal: true
                step: rail.pitch
                ease: railEase
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
            // A rail that fills while the page is up (the setup dialog, the add page) lands on its first game, as a cold start does.
            onCountChanged: {
                if (currentIndex < 0 && count > 0) {
                    currentIndex = 0;
                    page.tileSelected = false;
                }
                slideToCurrent();
            }

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
            Keys.onPressed: function (event) {
                if (!api.keys.isFirst(event) && !api.keys.isLast(event))
                    return;
                event.accepted = true;
                var at = page.tileSelected ? rail.count : rail.currentIndex;
                var to = api.keys.isFirst(event) ? 0 : rail.count;
                if (at === to || rail.count === 0) {
                    Sound.edge();
                    return;
                }
                Sound.tick();
                page.pointToTile(to);
            }

            Behavior on contentX {
                id: railEase
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

                    pointable: true
                    current: tile.selected && rail.activeFocus
                    onPicked: page.pointToTile(index)

                    width: page.cellSize
                    height: page.cellSize
                    anchors.bottom: parent.bottom
                    x: (parent.width - width) / 2 + (tile.delta === 0 ? 0 : (tile.delta < 0 ? -page.spread : page.spread))

                    game: model
                    artSource: model.id === page.playingId && api.home.frame !== "" ? api.home.frame : String(model.assets.square) !== "" ? model.assets.square : model.assets.boxFront
                    playing: model.id === page.playingId
                    arriving: model.installing
                    progress: model.progress
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
            width: Theme.dp(90 - 20)
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
            width: Theme.dp(90 - 20)
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
