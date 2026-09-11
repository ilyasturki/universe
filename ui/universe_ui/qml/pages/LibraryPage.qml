import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    signal chromeRequested()

    readonly property var currentGame: grid.currentIndex >= 0 && sorted.count > 0
                                       ? sorted.get(grid.currentIndex) : null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 18
    readonly property real chromeScrim: 0
    readonly property real scrimTop: 0.58
    readonly property real scrimMid: 0.74
    readonly property real scrimBottom: 0.95
    readonly property Item focusedArtItem: grid.focusedArtItem
    readonly property Item menuAnchor: chipBar.activeFocus ? null : grid.focusedArtItem
    // Read by tools/shot: up here A opens a list instead of launching a game.
    readonly property bool ownsAccept: chipBar.activeFocus

    readonly property var hints: picker.open
        ? picker.hints
        : chipBar.activeFocus
        ? [ { glyph: "A", label: "Change" },
            { glyph: "B", label: "Back to grid" },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "LB RB", label: "Tabs" } ]
        : [ { glyph: "A", label: "Launch" },
            { glyph: "X", label: "Details" },
            { glyph: "Y", label: "Sort" },
            { glyph: "LT RT", label: "Collection" },
            { glyph: "LB RB", label: "Tabs" } ]

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

    // Called by the shell when the tab is left or a game launches.
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
        grid.currentIndex = 0;
        grid.contentY = 0;
    }

    LibraryGames {
        id: sorted
        sourceModel: page.activeSource
        sortMode: page.sortMode
    }

    Item {
        id: header

        // The dropdown hangs over the grid, which is painted after this.
        z: 2

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: titleText.height + Theme.dp(14) + metaLine.height

        Text {
            id: titleText
            anchors.left: parent.left
            anchors.right: chipBar.left
            anchors.rightMargin: Theme.dp(40)
            text: page.currentGame ? page.currentGame.title : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(58)
            elide: Text.ElideRight
        }

        GameMetaLine {
            id: metaLine
            anchors.top: titleText.bottom
            anchors.topMargin: Theme.dp(14)
            anchors.left: parent.left
            game: page.currentGame
        }

        FocusScope {
            id: chipBar

            anchors.right: parent.right
            anchors.bottom: parent.bottom
            width: chips.width
            height: chips.height

            property int index: 0

            function step(d) {
                var next = Math.max(0, Math.min(1, index + d));
                if (next === index) {
                    Sound.edge();
                    return;
                }
                index = next;
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

                onChosen: {
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
            Keys.onDownPressed: {
                Sound.panel();
                grid.forceActiveFocus();
            }

            Keys.onPressed: {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    chipBar.openPicker();
                    return;
                }
                if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    grid.forceActiveFocus();
                    return;
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
        selectionActive: !chipBar.activeFocus

        Keys.onPressed: {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isFilters(event)) {
                event.accepted = true;
                page.cycleSort(1);
                return;
            }
            // Pegasus turns every trigger axis sample into a fresh press, so a
            // squeeze arrives as a burst of them and only its release is single.
            if (api.keys.isPageUp(event) || api.keys.isPageDown(event)) {
                event.accepted = true;
                return;
            }
        }

        Keys.onReleased: {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isPageUp(event)) {
                event.accepted = true;
                page.cycleCollection(-1);
                return;
            }
            if (api.keys.isPageDown(event)) {
                event.accepted = true;
                page.cycleCollection(1);
                return;
            }
        }
    }

    Keys.onUpPressed: {
        Sound.panel();
        chipBar.forceActiveFocus();
    }
}
