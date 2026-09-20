import QtQuick
import "../core"
import "../sound"

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
    readonly property real room: Theme.dp(Theme.ringRoom)
    readonly property real pitchY: cellHeight + gap

    signal escapedLeft
    signal activated
    signal optionsRequested

    width: columns * cellWidth + (columns - 1) * gap

    function move(d) {
        index = Sound.stepped(index, d, count);
    }
    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.floor(grid.height / pitchY), count);
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
    Keys.onRightPressed: move(1)
    Keys.onUpPressed: index >= columns ? move(-columns) : Sound.play("edge")
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
            Ease {}
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
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: -Theme.dp(96)
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        flickable: view
    }
}
