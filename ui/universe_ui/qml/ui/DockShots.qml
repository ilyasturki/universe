import QtQuick
import "../core"
import "../sound"

// ▼ from the dock's row: the playing game's screenshots over the whole frame, this session's first.
FocusScope {
    id: panel

    property var session: null
    property bool open: false
    property bool lightbox: false

    readonly property var store: api.screens.shots
    readonly property var current: grid.current
    readonly property int count: grid.ordered.length
    readonly property int mine: grid.mine
    readonly property bool busy: confirm.open
    readonly property var hints: lightbox ? [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ]
        : [ { glyph: "A", label: "View", dim: current === null },
            { glyph: "Y", label: "Remove", dim: current === null },
            { glyph: "B", label: "Dock" } ]

    signal closeRequested()

    function load() {
        if (session && session.id)
            store.load(session.id);
    }

    function view() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.enter();
        lightbox = true;
    }

    function remove() {
        if (!current) {
            Sound.edge();
            return;
        }
        var row = current;
        confirm.ask({ message: "Remove this screenshot?", detail: row.dateText + ". The picture goes to the trash" + (row.hasJournal ? "; its journal entry keeps the rest." : "."),
                      yes: "Trash the screenshot", no: "Keep it", index: 0 },
                    function(yes) { if (yes) store.remove(row.gameId, row.name); });
    }

    function close() {
        Sound.cancel();
        lightbox = false;
        panel.closeRequested();
    }

    onOpenChanged: {
        if (open) {
            grid.index = 0;
            lightbox = false;
            load();
            forceActiveFocus();
        }
    }

    onSessionChanged: if (open) load()

    Connections {
        target: panel.store
        function onRowsChanged() {
            if (grid.index >= grid.ordered.length)
                grid.index = Math.max(0, grid.ordered.length - 1);
        }
    }

    y: open ? 0 : height
    visible: y < height

    Behavior on y { Ease { duration: Theme.durScene } }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: grid.sideMargin
        anchors.rightMargin: grid.sideMargin
        height: title.height

        Text {
            id: title
            anchors.left: parent.left
            anchors.right: parent.right
            text: panel.session ? panel.session.title : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(46)
            elide: Text.ElideRight
        }
    }

    ShotGrid {
        id: grid

        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        anchors.left: parent.left
        anchors.right: parent.right
        topPadding: Theme.dp(34)
        rows: panel.store.rows
        since: panel.session && panel.session.started_at ? panel.session.started_at : ""
        active: panel.open && !panel.lightbox
    }

    Lightbox {
        anchors.fill: parent
        z: 4
        images: grid.ordered.map(function(r) { return r.url; })
        index: grid.index
        open: panel.lightbox
    }

    HintBar {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: grid.sideMargin
        hints: panel.busy ? confirm.hints : panel.hints
        z: 4
    }

    ConfirmDialog {
        id: confirm
        anchors.fill: parent
        z: 6
        onClosed: panel.forceActiveFocus()
    }

    Keys.onPressed: function(event) {
        if (!panel.open || confirm.open) {
            event.accepted = panel.open;
            return;
        }
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        event.accepted = true;
        if (event.isAutoRepeat && !arrow && !vertical)
            return;
        if (lightbox) {
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                lightbox = false;
            } else if (arrow)
                grid.step(event.key === Qt.Key_Left ? -1 : 1);
            return;
        }
        if (api.keys.isAccept(event))
            view();
        else if (api.keys.isCancel(event))
            close();
        else if (api.keys.isFilters(event))
            remove();
        else if (arrow)
            grid.step(event.key === Qt.Key_Left ? -1 : 1);
        else if (vertical) {
            var up = event.key === Qt.Key_Up;
            if (!grid.stepLine(up ? -1 : 1)) {
                if (up)
                    close();
                else
                    Sound.edge();
            }
        }
    }
}
