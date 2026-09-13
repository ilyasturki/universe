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
    signal chromeRequested()

    readonly property int railFloor: 5
    readonly property bool standingIn: (recent ? recent.count : 0) < railFloor
    readonly property var railModel: standingIn ? newest : recent
    readonly property int railCount: railModel ? railModel.count : 0

    readonly property var currentGame: railCount > 0 && rail.currentIndex >= 0
                                       ? railModel.get(rail.currentIndex) : null
    // The shell paints the hero art behind this band, and stands its blurred stage down.
    readonly property bool ownsBackdrop: true
    readonly property real backdropBlur: 0
    readonly property real chromeScrim: 0
    readonly property real scrimTop: 0
    readonly property real scrimMid: 0
    readonly property real scrimBottom: 0
    readonly property Item focusedArtItem: rail.currentItem ? rail.currentItem.artItem : null
    // Start acts on the card whether the rail or the hero actions hold focus; the Library tile is no game.
    readonly property Item menuAnchor: tileSelected ? null : focusedArtItem
    // Read by tools/shot: the tile and the Details button consume A themselves.
    readonly property bool ownsAccept: tileSelected || (heroActions.activeFocus && heroActions.index === 1)
    // Set by the shell; the rail keeps its ring lit while the menu holds focus.
    property bool menuOpen: false

    // The Library tile sits past the last card. The rail's index stays where it
    // was, so the hero art behind the Library hero is the last game's.
    property bool tileSelected: false
    readonly property int focusIndex: tileSelected ? railCount : rail.currentIndex

    readonly property var hints: {
        var out = [];
        if (tileSelected) {
            out.push({ glyph: "A", label: "Open library" });
        } else {
            out.push({ glyph: "A", label: heroActions.activeFocus && heroActions.index === 1 ? "Details" : playLabel });
            if (!heroActions.activeFocus)
                out.push({ glyph: "X", label: "Details" });
            out.push({ glyph: "Y", label: favouriteLabel });
        }
        if (heroActions.activeFocus)
            out.push({ glyph: "B", label: "Back to games" });
        out.push({ glyph: "LB RB", label: "Tabs" });
        out.push({ glyph: "dpad", label: "Navigate" });
        return out;
    }

    readonly property string playLabel: currentGame && currentGame.playTime > 0 ? "Continue" : "Play"
    readonly property string favouriteLabel: currentGame && currentGame.favorite ? "Remove from favourites" : "Add to favourites"

    readonly property real bandHeight: Theme.dp(Theme.heroBand)
    readonly property real railGapTop: Theme.dp(28)
    // Cards are authored at 240 and scaled to 176 when idle, but the slot is the
    // idle size — the focused card overhangs it and its neighbours step aside,
    // which is what keeps every gap at 24 whatever has focus.
    readonly property real cellSize: Theme.dp(240)
    readonly property real slotSize: Theme.dp(176)
    readonly property real railGap: Theme.dp(24)
    readonly property real spread: (cellSize - slotSize) / 2
    readonly property real idleScale: 176 / 240

    property int libraryCount: 0
    property int librarySeconds: 0

    function refreshLibraryStats() {
        var seconds = 0;
        for (var i = 0; i < api.allGames.count; i++)
            seconds += api.allGames.get(i).playTime;
        libraryCount = api.allGames.count;
        librarySeconds = seconds;
    }

    function toggleFavourite() {
        if (!currentGame || tileSelected) {
            Sound.edge();
            return;
        }
        currentGame.favorite = !currentGame.favorite;
        Sound.favourite(currentGame.favorite);
    }

    Component.onCompleted: refreshLibraryStats()

    onTileSelectedChanged: {
        if (tileSelected)
            refreshLibraryStats();
        heroActions.index = Math.min(heroActions.index, heroActions.last);
        rail.slideToCurrent();
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            toggleFavourite();
        }
    }

    RecentGames {
        id: recent
        sourceModel: api.allGames
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
            // By residue, so a slot keeps its logo across a step and never has a
            // decode in flight handed to another slot.
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
                NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
            }

            HeroLogo {
                id: heroLogo
                game: page.currentGame
                titleWidth: parent.width
                opacity: page.tileSelected ? 0.0 : 1.0

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                }
            }

            Text {
                anchors.bottom: heroLogo.bottom
                text: "Library"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(58)
                opacity: page.tileSelected ? 1.0 : 0.0

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                }
            }

            Text {
                anchors.verticalCenter: heroMeta.verticalCenter
                text: page.libraryCount + (page.libraryCount === 1 ? " game · " : " games · ")
                      + Format.totalPlayTime(page.librarySeconds) + " played"
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
                opacity: page.tileSelected ? 1.0 : 0.0

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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
                    var next = Math.max(0, Math.min(last, index + d));
                    if (next === index) {
                        Sound.edge();
                        return;
                    }
                    index = next;
                    Sound.tick();
                }

                Row {
                    id: buttons
                    // Clears the focused pill's ring, which reaches 20 past its edge.
                    spacing: Theme.dp(36)
                    opacity: page.tileSelected ? 0.0 : 1.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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
                    icon: "library"
                    label: "Open library"
                    focused: heroActions.activeFocus && page.tileSelected
                    opacity: page.tileSelected ? 1.0 : 0.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                    }
                }

                Keys.onLeftPressed: heroActions.step(-1)
                Keys.onRightPressed: heroActions.step(1)
                Keys.onUpPressed: page.chromeRequested()
                Keys.onDownPressed: function(event) {
                    Sound.panel();
                    rail.forceActiveFocus();
                }

                Keys.onPressed: function(event) {
                    if (api.keys.isAccept(event)) {
                        if (page.tileSelected) {
                            event.accepted = true;
                            if (!event.isAutoRepeat)
                                page.tabRequested(1);
                            return;
                        }
                        if (heroActions.index === 1) {
                            event.accepted = true;
                            page.detailRequested(page.currentGame);
                            return;
                        }
                        // Play falls through to the shell, which launches on release.
                    }
                    if (api.keys.isDetails(event) && page.tileSelected) {
                        event.accepted = true;
                        Sound.edge();
                        return;
                    }
                    if (api.keys.isCancel(event)) {
                        event.accepted = true;
                        Sound.cancel();
                        rail.forceActiveFocus();
                        return;
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
            visible: page.standingIn
            text: "FROM YOUR LIBRARY · RECENTLY PLAYED GOES HERE"
        }

        ListView {
            id: rail

            anchors.top: parent.top
            anchors.topMargin: page.railGapTop + (page.standingIn ? standInNote.height + Theme.dp(14) : 0)
            anchors.left: parent.left
            anchors.right: parent.right
            // The focused card overhangs its slot by `spread` at both ends. Absorbing
            // the left overhang into the margin keeps contentX >= 0, which a
            // ListView will honour; a header moves originX negative and the view
            // then refuses to scroll the whole way to it.
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
            // The default overshoot fixup fights the contentX Behavior and settles
            // short of originX, leaving the first card misaligned.
            boundsBehavior: Flickable.StopAtBounds
            highlightFollowsCurrentItem: false
            cacheBuffer: page.cellSize * 3

            readonly property real pitch: page.slotSize + page.railGap

            // The Library tile lives in the footer: it extends contentWidth without
            // disturbing originX, and it is not a game the model has to carry.
            footer: Item {
                width: page.railGap + page.slotSize + page.spread * 2
                height: page.cellSize

                LibraryTile {
                    width: page.cellSize
                    height: page.cellSize
                    anchors.bottom: parent.bottom
                    x: page.railGap + (page.slotSize - width) / 2 + (page.tileSelected ? 0 : page.spread)
                    selected: page.tileSelected
                    idleScale: page.idleScale
                    count: page.libraryCount
                    ringOpacity: rail.activeFocus || page.menuOpen ? 1.0 : Theme.ringIdle

                    Behavior on x {
                        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
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

            // Left unaccepted so the view's own navigation still moves the index.
            Keys.onLeftPressed: function(event) {
                if (page.tileSelected) {
                    page.tileSelected = false;
                    Sound.tick();
                    return;
                }
                event.accepted = false;
                rail.currentIndex > 0 ? Sound.tick() : Sound.edge();
            }
            Keys.onRightPressed: function(event) {
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
            Keys.onUpPressed: function(event) {
                Sound.panel();
                heroActions.forceActiveFocus();
            }
            Keys.onDownPressed: Sound.edge()

            // On the tile, A opens the library and X has nothing to show; neither
            // may reach the shell, which would act on the last game.
            Keys.onPressed: function(event) {
                if (!page.tileSelected)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    if (!event.isAutoRepeat)
                        page.tabRequested(1);
                    return;
                }
                if (api.keys.isDetails(event)) {
                    event.accepted = true;
                    Sound.edge();
                    return;
                }
            }

            Behavior on contentX {
                NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint }
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
                    x: (parent.width - width) / 2
                       + (tile.delta === 0 ? 0 : (tile.delta < 0 ? -page.spread : page.spread))

                    game: model
                    artSource: String(model.assets.tile) !== "" ? model.assets.tile : model.assets.boxFront
                    selected: tile.selected
                    selectedScale: 1.0
                    idleScale: page.idleScale
                    focusOrigin: Item.Bottom
                    cornerRadius: Theme.dp(Theme.radiusTile)
                    ringOpacity: rail.activeFocus || page.menuOpen ? 1.0 : Theme.ringIdle

                    Behavior on x {
                        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                    }
                }
            }
        }

        // The rail runs past the margin on both sides; without these it is
        // guillotined at the screen edge.
        Rectangle {
            anchors.left: parent.left
            anchors.top: rail.top
            anchors.bottom: rail.bottom
            // Stops short of where the focused ring's halo sits at either end.
            width: Theme.dp(90 - 14)
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: Theme.ground }
                GradientStop { position: 1.0; color: Qt.rgba(0.055, 0.059, 0.075, 0.0) }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.top: rail.top
            anchors.bottom: rail.bottom
            width: Theme.dp(90 - 14)
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: Qt.rgba(0.055, 0.059, 0.075, 0.0) }
                GradientStop { position: 1.0; color: Theme.ground }
            }
        }
    }
}
