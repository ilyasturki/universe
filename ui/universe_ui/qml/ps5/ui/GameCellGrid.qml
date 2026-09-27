import QtQuick
import "../core"
import "../sound"

// A grid of cards the cursor walks row by row; the delegate reads `entry`, `focused` and `index`.
FocusScope {
    id: grid

    property var model: []
    property int columns: 3
    property real cellWidth: 0
    property real cellHeight: 0
    property real gap: 0
    property int index: 0
    property Component delegate: null
    readonly property int count: model.length
    readonly property int lastRow: count > 0 ? Math.floor((count - 1) / columns) : 0
    // What a clipping view keeps past its cards so the focus ring is never cut.
    readonly property real room: Theme.dp(Theme.ringGap + Theme.ringLine + 8)
    readonly property real pitchY: cellHeight + gap

    signal escapedLeft
    signal escapedUp
    signal activated
    signal optionsRequested
    signal pointed

    width: columns * cellWidth + (columns - 1) * gap

    function move(d) {
        index = Sound.stepped(index, d, count);
    }
    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.floor(grid.height / pitchY), count);
    }
    function point(i) {
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

    Keys.onLeftPressed: {
        if (index % columns === 0) {
            Sound.play("tick");
            grid.escapedLeft();
        } else {
            move(-1);
        }
    }
    Keys.onRightPressed: index % columns < columns - 1 ? move(1) : Sound.play("edge")
    Keys.onUpPressed: {
        if (index >= columns) {
            move(-columns);
        } else {
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
        anchors.margins: -grid.room
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

            Loader {
                readonly property var entry: modelData
                readonly property bool focused: grid.activeFocus && index === grid.index

                x: grid.room + (index % grid.columns) * (grid.cellWidth + grid.gap)
                y: grid.room + Math.floor(index / grid.columns) * grid.pitchY
                width: grid.cellWidth
                height: grid.cellHeight
                z: focused ? 2 : 1
                sourceComponent: grid.delegate

                Touch {
                    current: parent.focused
                    menu: true
                    onPicked: grid.point(index)
                }
            }
        }
    }

    Swipe {
        flickable: view
        ease: scrollEase
    }

    Scrollbar {
        anchors.left: parent.right
        anchors.leftMargin: Theme.dp(40)
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        flickable: view
    }
}
