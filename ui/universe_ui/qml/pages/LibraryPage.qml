import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    signal chromeRequested()
    signal addRequested()

    readonly property var currentGame: anchor.game
    readonly property bool onAddTile: grid.addSelected
    readonly property bool empty: api.allGames.count === 0
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 18
    readonly property real scrimTop: 0.58
    readonly property real scrimMid: 0.74
    readonly property real scrimBottom: 0.95
    readonly property Item menuAnchor: grid.focusedArtItem
    readonly property bool modal: picker.open
    property bool menuOpen: false

    readonly property var hints: picker.open
        ? picker.hints
        : chipBar.activeFocus
        ? [ { glyph: "A", label: "Change" },
            { glyph: "B", label: "Back to grid" },
            { glyph: "LT RT", label: "Collection" } ]
        : onAddTile
        ? [ { glyph: "A", label: "Add a game" },
            { glyph: "Y", label: "Sort" },
            { glyph: "LT RT", label: "Collection" } ]
        : [ { glyph: "A", label: "Launch" },
            { glyph: "X", label: "Details" },
            { glyph: "Y", label: "Sort" },
            { glyph: "LT RT", label: "Collection" } ]

    function cycleCollection(step) {
        var n = api.collections.count + 1;
        collectionIndex = (collectionIndex + step + n) % n;
        Sound.collection();
    }

    function cycleSort(step) {
        sortMode = (sortMode + step + sortNames.length) % sortNames.length;
        Sound.sort();
    }

    function collectionOptions() {
        var out = [ { label: "All collections", trailing: api.allGames.count.toString() } ];
        for (var i = 0; i < api.collections.count; i++) {
            var c = api.collections.get(i);
            out.push({ label: c.name, trailing: c.games.count.toString() });
        }
        return out;
    }

    function sortOptions() {
        return sortNames.map(function(name) { return { label: name }; });
    }

    function leave() {
        picker.hide();
    }

    readonly property var sortNames: ["Last played", "Title", "Playtime", "Released"]
    property int sortMode: 0
    property int collectionIndex: 0

    readonly property var activeSource: collectionIndex === 0
                                        ? api.allGames
                                        : api.collections.get(collectionIndex - 1).games
    readonly property string collectionLabel: collectionIndex === 0
                                              ? "All collections"
                                              : api.collections.get(collectionIndex - 1).name

    readonly property int columns: 8
    readonly property real gap: Theme.dp(28)
    readonly property real sideMargin: Theme.dp(80)
    readonly property real gridMargin: sideMargin - gap / 2
    readonly property real cellWidth: (width - gridMargin * 2) / columns

    Component.onCompleted: {
        if (api.memory.has("librarySort"))
            sortMode = api.memory.get("librarySort");
        if (api.memory.has("libraryCollection"))
            collectionIndex = Math.min(api.memory.get("libraryCollection"), api.collections.count);
    }
    onSortModeChanged: api.memory.set("librarySort", sortMode)
    onCollectionIndexChanged: {
        api.memory.set("libraryCollection", collectionIndex);
        grid.addPicked = false;
        grid.currentIndex = 0;
        grid.scrollToCurrent();
    }

    LibraryGames {
        id: sorted
        sourceModel: page.activeSource
        sortMode: page.sortMode
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: sorted
        index: grid.addSelected ? -1 : grid.currentIndex
        onMoved: function(index) { grid.addPicked = false; grid.currentIndex = index; }
    }

    Item {
        id: header

        z: 2

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: titleText.height + Theme.dp(14) + metaSlot.height

        Text {
            id: titleText
            anchors.left: parent.left
            anchors.right: chipBar.left
            anchors.rightMargin: Theme.dp(40)
            text: page.currentGame ? page.currentGame.title : page.onAddTile ? (page.empty ? "Add your first game" : "Add a game") : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(58)
            elide: Text.ElideRight
        }

        Item {
            id: metaSlot

            anchors.top: titleText.bottom
            anchors.topMargin: Theme.dp(14)
            anchors.left: parent.left
            // The platform icon's height: the line must not resize the grid as the cursor moves.
            height: Theme.dp(28)

            GameMetaLine {
                id: metaLine
                anchors.verticalCenter: parent.verticalCenter
                game: page.currentGame
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: page.onAddTile
                text: page.empty ? "Nothing in the library yet: a file on this machine, a store, or your Lutris games."
                                 : "A file on this machine, a store, or your Lutris games."
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
            }
        }

        FocusScope {
            id: chipBar

            anchors.right: parent.right
            anchors.bottom: parent.bottom
            width: chips.width
            height: chips.height

            property int index: 0

            function step(d) {
                index = (index + d + 2) % 2;
                Sound.tick();
            }

            function openPicker() {
                Sound.panel();
                if (index === 0)
                    picker.show(collectionChip, page.collectionOptions(), page.collectionIndex);
                else
                    picker.show(sortChip, page.sortOptions(), page.sortMode);
            }

            Row {
                id: chips
                spacing: Theme.dp(14)

                Chip {
                    id: collectionChip
                    label: page.collectionLabel
                    trailing: sorted.count.toString()
                    focused: chipBar.activeFocus && chipBar.index === 0
                }

                Chip {
                    id: sortChip
                    label: page.sortNames[page.sortMode]
                    showSortIcon: true
                    focused: chipBar.activeFocus && chipBar.index === 1
                }
            }

            ChipPicker {
                id: picker

                onChosen: function(index) {
                    picker.hide();
                    chipBar.forceActiveFocus();
                    if (chipBar.index === 0) {
                        page.collectionIndex = index;
                        Sound.collection();
                    } else {
                        page.sortMode = index;
                        Sound.sort();
                    }
                }
                onDismissed: {
                    picker.hide();
                    chipBar.forceActiveFocus();
                }
            }

            Keys.onLeftPressed: chipBar.step(-1)
            Keys.onRightPressed: chipBar.step(1)
            Keys.onUpPressed: page.chromeRequested()
            Keys.onDownPressed: function(event) {
                Sound.panel();
                grid.forceActiveFocus();
            }

            Keys.onPressed: function(event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    chipBar.openPicker();
                } else if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    grid.forceActiveFocus();
                }
            }
        }
    }

    CoverGrid {
        id: grid

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(34) - grid.inset
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.gridMargin - grid.inset
        anchors.rightMargin: page.gridMargin - grid.inset

        focus: true
        model: sorted
        columns: page.columns
        gap: page.gap
        cellWidth: page.cellWidth
        selectionActive: grid.activeFocus || page.menuOpen
        addTile: true

        Keys.onPressed: function(event) {
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
                page.cycleSort(1);
            } else if (api.keys.isPageUp(event) || api.keys.isPageDown(event)) {
                // Pegasus turns every trigger axis sample into a fresh press; only the release is single.
                event.accepted = true;
            }
        }

        Keys.onReleased: function(event) {
            if (event.isAutoRepeat)
                return;
            var d = api.keys.isPageUp(event) ? -1 : api.keys.isPageDown(event) ? 1 : 0;
            if (!d)
                return;
            event.accepted = true;
            page.cycleCollection(d);
        }
    }

    Keys.onUpPressed: function(event) {
        Sound.panel();
        chipBar.forceActiveFocus();
    }
}
