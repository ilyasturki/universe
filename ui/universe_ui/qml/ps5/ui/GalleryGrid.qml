import QtQuick
import "../core"
import "../sound"

// The Media Gallery's grid: three captures a row, the focused row kept in view.
FocusScope {
    id: grid

    // Rows of api.screens.media.
    property var model: []
    property int columns: 3
    property real cellWidth: 0
    property real cellHeight: 0
    property real gapX: 0
    property real gapY: 0
    property int index: 0
    // Room above and below inside the clip for the focused card's ring.
    readonly property real room: Theme.dp(10)
    readonly property int count: model.length
    readonly property int lastRow: count > 0 ? Math.floor((count - 1) / columns) : 0
    readonly property real pitchY: cellHeight + gapY
    readonly property bool cursorShown: activeFocus

    signal escapedLeft
    signal escapedUp
    signal activated
    signal optionsRequested
    signal pointed

    function move(d) {
        index = Sound.stepped(index, d, count);
    }

    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.max(1, Math.floor(view.height / pitchY)), count);
    }

    function point(i) {
        if (i === index && activeFocus) {
            grid.activated();
            return;
        }
        Sound.play("tick");
        index = i;
        if (!activeFocus)
            grid.pointed();
        forceActiveFocus();
    }

    function scrollToCurrent() {
        if (view.height <= 0)
            return;
        var top = Math.floor(index / columns) * pitchY;
        Theme.reveal(view, top, top + cellHeight + room * 2, view.height);
    }

    onCountChanged: {
        if (index >= count)
            index = Math.max(0, count - 1);
        scrollToCurrent();
    }
    onIndexChanged: scrollToCurrent()
    onHeightChanged: scrollToCurrent()

    Keys.onLeftPressed: {
        if (index % columns === 0) {
            Sound.play("tick");
            grid.escapedLeft();
        } else {
            move(-1);
        }
    }
    Keys.onRightPressed: {
        if (index % columns === columns - 1 || index === count - 1)
            Sound.play("edge");
        else
            move(1);
    }
    Keys.onUpPressed: {
        if (index >= columns)
            move(-columns);
        else {
            Sound.play("tick");
            grid.escapedUp();
        }
    }
    Keys.onDownPressed: Math.floor(index / columns) < lastRow ? move(Math.min(columns, count - 1 - index)) : Sound.play("edge")

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            stepScreen(screen);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            grid.activated();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            grid.optionsRequested();
        }
    }

    Flickable {
        id: view

        anchors.fill: parent
        anchors.topMargin: -grid.room
        anchors.leftMargin: -grid.room
        anchors.rightMargin: -grid.room
        contentWidth: width
        contentHeight: (grid.lastRow + 1) * grid.pitchY + grid.room * 2
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Repeater {
            model: grid.model

            GalleryCard {
                x: grid.room + (index % grid.columns) * (grid.cellWidth + grid.gapX)
                y: grid.room + Math.floor(index / grid.columns) * grid.pitchY
                width: grid.cellWidth
                height: grid.cellHeight
                z: focused ? 2 : 1
                row: modelData
                focused: grid.cursorShown && index === grid.index
                onPicked: grid.point(index)
            }
        }
    }

    Swipe {
        flickable: view
        ease: scrollEase
    }

    Scrollbar {
        anchors.left: parent.right
        anchors.leftMargin: Theme.dp(30)
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        flickable: view
    }
}
