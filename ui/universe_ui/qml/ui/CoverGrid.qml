import QtQuick
import "../core"
import "../sound"

GridView {
    id: grid

    property int columns: 8
    property real gap: Theme.dp(28)
    property bool selectionActive: true
    // One more cell after the last game: the tile that adds one. It holds the cursor as `addPicked`, never `currentIndex`.
    property bool addTile: false
    property bool addPicked: false
    readonly property bool addSelected: addPicked || (addTile && count === 0)
    readonly property real coverWidth: cellWidth - gap
    readonly property Item focusedArtItem: currentItem && !addSelected ? currentItem.artItem : null
    readonly property int cells: count + (addTile ? 1 : 0)
    readonly property int cursor: addSelected ? count : currentIndex
    // Room inside the clip for the focused cover's ring and its 5% growth.
    readonly property real inset: Theme.dp(12)

    cellHeight: coverWidth * 1.5 + gap
    topMargin: inset
    bottomMargin: inset
    leftMargin: inset
    rightMargin: inset
    interactive: false
    // Built-in navigation accepts Up on the top row without moving, swallowing it.
    keyNavigationEnabled: false
    highlightFollowsCurrentItem: false
    clip: true
    cacheBuffer: cellHeight * 2

    readonly property int lastRow: cells > 0 ? Math.floor((cells - 1) / columns) : 0
    readonly property int visibleRows: cellHeight > 0 ? Math.max(1, Math.floor(height / cellHeight)) : 1

    function targetY(index) {
        // A Flickable whose content fits rests at -topMargin, not 0.
        if (contentHeight + topMargin + bottomMargin <= height)
            return -topMargin;
        var rowTop = Math.floor(index / columns) * cellHeight;
        var target = contentY;
        if (rowTop - inset < contentY)
            target = rowTop - inset;
        else if (rowTop + cellHeight + inset > contentY + height)
            target = rowTop + cellHeight + inset - height;
        var maxY = contentHeight - height + bottomMargin;
        return Math.max(-topMargin, Math.min(target, maxY));
    }

    function scrollToCurrent() {
        scroller.stop();
        if (height <= 0 || cellHeight <= 0)
            return;
        // Not `cursor`: its binding still holds the old index inside onCurrentIndexChanged.
        contentY = targetY(addSelected ? count : currentIndex);
    }

    // Setting currentIndex moves contentY synchronously, past any Behavior: snapshot, restore, animate.
    function moveCurrent(index) {
        if (index < 0 || index >= cells || index === cursor) {
            Sound.edge();
            return;
        }
        Sound.tick();
        var from = contentY;
        if (index === count) {
            addPicked = true;
            contentY = targetY(index);
        } else {
            addPicked = false;
            currentIndex = index;
        }
        var to = contentY;
        if (Math.abs(to - from) < 0.5)
            return;
        contentY = from;
        scroller.from = from;
        scroller.to = to;
        scroller.start();
    }

    NumberAnimation {
        id: scroller
        target: grid
        property: "contentY"
        duration: Theme.durView
        easing.type: Easing.OutQuint
    }

    onCurrentIndexChanged: scrollToCurrent()
    onHeightChanged: scrollToCurrent()
    onCountChanged: scrollToCurrent()

    Keys.onDownPressed: function (event) {
        if (!selectionActive)
            event.accepted = false;
        else if (Math.floor(cursor / columns) < lastRow)
            moveCurrent(Math.min(cursor + columns, cells - 1));
        else
            Sound.edge();
    }

    Keys.onUpPressed: function (event) {
        if (selectionActive && cursor >= columns)
            moveCurrent(cursor - columns);
        else
            event.accepted = false;
    }

    Keys.onLeftPressed: function (event) {
        if (!selectionActive)
            event.accepted = false;
        else
            moveCurrent(cursor - 1);
    }

    Keys.onRightPressed: function (event) {
        if (!selectionActive)
            event.accepted = false;
        else
            moveCurrent(cursor + 1);
    }

    // The right stick: a screenful of rows, staying in the column.
    Keys.onPressed: function (event) {
        var d = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (!d || !selectionActive)
            return;
        event.accepted = true;
        var row = Math.max(0, Math.min(lastRow, Math.floor(cursor / columns) + d * visibleRows));
        moveCurrent(Math.min(cells - 1, row * columns + cursor % columns));
    }

    // Room for the add tile, drawn outside the delegates, when it starts a row.
    footer: Item {
        width: 1
        height: grid.addTile && grid.count % grid.columns === 0 ? grid.cellHeight : 0
    }

    Item {
        // A GridView keeps its own children on the view; only contentItem scrolls with the cells.
        parent: grid.contentItem
        visible: grid.addTile
        x: (grid.count % grid.columns) * grid.cellWidth
        y: Math.floor(grid.count / grid.columns) * grid.cellHeight
        width: grid.cellWidth
        height: grid.cellHeight

        LibraryTile {
            anchors.centerIn: parent
            width: grid.coverWidth
            height: grid.coverWidth * 1.5
            kind: "add"
            cornerRadius: Theme.dp(Theme.radiusCover)
            selected: grid.addSelected
            ringOpacity: grid.selectionActive ? 1.0 : Theme.ringIdle
        }
    }

    delegate: Item {
        id: cell

        readonly property bool selected: GridView.isCurrentItem && !grid.addSelected
        property alias artItem: cover

        width: grid.cellWidth
        height: grid.cellHeight

        CoverCard {
            id: cover
            anchors.centerIn: parent
            width: grid.coverWidth
            height: grid.coverWidth * 1.5
            game: model
            selected: cell.selected
            ringOpacity: grid.selectionActive ? 1.0 : Theme.ringIdle
        }
    }
}
