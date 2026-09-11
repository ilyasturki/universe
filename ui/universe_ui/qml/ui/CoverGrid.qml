import QtQuick
import "../core"
import "../sound"

GridView {
    id: grid

    property int columns: 8
    property real gap: Theme.dp(28)
    // While the selection is inactive another group owns the d-pad; keys fall through.
    property bool selectionActive: true
    property bool escapesLeft: false
    readonly property real coverWidth: cellWidth - gap
    readonly property Item focusedArtItem: currentItem ? currentItem.artItem : null
    // Room inside the clip for the focused cover's ring and its 5% growth; callers
    // widen their anchors by this so the cells stay put.
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

    readonly property int lastRow: count > 0 ? Math.floor((count - 1) / columns) : 0

    function scrollToCurrent() {
        scroller.stop();
        if (height <= 0 || cellHeight <= 0)
            return;
        if (contentHeight + topMargin + bottomMargin <= height) {
            // A Flickable whose content fits rests at -topMargin, not 0.
            contentY = -topMargin;
            return;
        }
        var rowTop = Math.floor(currentIndex / columns) * cellHeight;
        var target = contentY;
        if (rowTop - inset < contentY)
            target = rowTop - inset;
        else if (rowTop + cellHeight + inset > contentY + height)
            target = rowTop + cellHeight + inset - height;
        var maxY = contentHeight - height + bottomMargin;
        contentY = Math.max(-topMargin, Math.min(target, maxY));
    }

    // Setting currentIndex moves contentY synchronously, past any Behavior —
    // snapshot it, restore, animate.
    function moveCurrent(index) {
        if (index < 0 || index >= count || index === currentIndex) {
            Sound.edge();
            return;
        }
        Sound.tick();
        var from = contentY;
        currentIndex = index;
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

    // A ragged last row is short, so the step down clamps to the final cell.
    Keys.onDownPressed: function(event) {
        if (!selectionActive)
            event.accepted = false;
        else if (Math.floor(currentIndex / columns) < lastRow)
            moveCurrent(Math.min(currentIndex + columns, count - 1));
        else
            Sound.edge();
    }

    Keys.onUpPressed: function(event) {
        if (selectionActive && currentIndex >= columns)
            moveCurrent(currentIndex - columns);
        else
            event.accepted = false;
    }

    Keys.onLeftPressed: function(event) {
        if (!selectionActive || (escapesLeft && currentIndex % columns === 0))
            event.accepted = false;
        else
            moveCurrent(currentIndex - 1);
    }

    Keys.onRightPressed: function(event) {
        if (!selectionActive)
            event.accepted = false;
        else
            moveCurrent(currentIndex + 1);
    }

    delegate: Item {
        id: cell

        readonly property bool selected: GridView.isCurrentItem
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
