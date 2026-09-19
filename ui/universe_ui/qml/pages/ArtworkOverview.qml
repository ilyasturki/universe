import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: view

    objectName: "artworkOverview"

    readonly property var store: api.screens.artworkOverview
    readonly property var rows: store.rows
    readonly property var columns: store.columns

    signal openRequested(var game, string slot)
    signal fetchRequested(int games)
    signal escapedLeft()
    signal escapedUp()
    signal message(string text)

    property int row: 0
    property int col: 0
    property bool onButton: false
    readonly property var currentRow: row >= 0 && row < rows.length ? rows[row] : null
    readonly property var currentColumn: col >= 0 && col < columns.length ? columns[col] : null
    readonly property var job: store.job
    readonly property bool fetching: job !== null && job !== undefined && (job.ok === null || job.ok === undefined)

    readonly property real titleWidth: Theme.dp(300)
    readonly property real thumbHeight: Theme.dp(96)
    readonly property real rowHeight: Theme.dp(110)
    readonly property real cellGap: Theme.dp(18)

    readonly property bool stopping: fetching && job.cancelled
    readonly property string buttonLabel: stopping ? "Stopping…"
                                        : fetching ? "Stop" + (job.total > 0 ? " · " + (job.done + 1) + "/" + job.total : "")
                                        : store.missingGames === 0 ? "Nothing missing" : "Fetch missing art"
    readonly property bool buttonDim: stopping || (!fetching && store.missingGames === 0)

    readonly property var hints: [
        onButton ? { glyph: "A", label: fetching ? "Stop" : "Fetch", dim: buttonDim }
                 : { glyph: "A", label: currentRow && currentColumn ? "Open " + currentColumn.label.toLowerCase() : "Open", dim: currentRow === null },
        { glyph: "B", label: "Sections" }
    ]

    function load() {
        store.load();
    }

    onRowsChanged: if (row >= rows.length) row = Math.max(0, rows.length - 1)

    Component.onDestruction: store.unload()

    function widthOf(column) {
        return Math.round(view.thumbHeight * column.aspect);
    }

    function activate() {
        if (onButton) {
            if (fetching)
                store.cancelRefresh() ? Sound.cancel() : Sound.edge();
            else if (buttonDim)
                Sound.edge();
            else
                view.fetchRequested(store.missingGames);
            return;
        }
        var game = currentRow ? api.allGames.byId(currentRow.id) : null;
        if (!game || !currentColumn) {
            Sound.edge();
            return;
        }
        view.openRequested(game, currentColumn.slot);
    }

    function move(dr, dc) {
        var r = row + dr, c = col + dc;
        if (r < 0 || r >= rows.length || c < 0 || c >= columns.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        row = r;
        col = c;
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat && (api.keys.isAccept(event) || api.keys.isCancel(event)))
            return;
        event.accepted = true;
        if (api.keys.isAccept(event)) {
            activate();
        } else if (api.keys.isCancel(event)) {
            Sound.cancel();
            view.escapedLeft();
        } else if (onButton) {
            if (event.key === Qt.Key_Down && rows.length > 0) {
                Sound.tick();
                onButton = false;
            } else if (event.key === Qt.Key_Up) {
                view.escapedUp();
            } else if (event.key === Qt.Key_Left) {
                view.escapedLeft();
            } else if (event.key === Qt.Key_Right || event.key === Qt.Key_Down) {
                Sound.edge();
            } else {
                event.accepted = false;
            }
        } else if (event.key === Qt.Key_Up) {
            if (row === 0) {
                Sound.tick();
                onButton = true;
            } else {
                move(-1, 0);
            }
        } else if (event.key === Qt.Key_Down) {
            move(1, 0);
        } else if (event.key === Qt.Key_Left) {
            col === 0 ? view.escapedLeft() : move(0, -1);
        } else if (event.key === Qt.Key_Right) {
            move(0, 1);
        } else {
            event.accepted = false;
        }
    }

    Connections {
        target: view.store
        function onMessage(text) { view.message(text); }
    }

    Item {
        id: head

        width: parent.width
        height: fetchButton.height

        PillButton {
            id: fetchButton
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            ghost: true
            icon: ""
            label: view.buttonLabel
            focused: view.onButton && view.activeFocus
            dimmed: view.buttonDim
        }

        Row {
            x: view.titleWidth + view.cellGap
            anchors.bottom: parent.bottom
            spacing: view.cellGap

            Repeater {
                model: view.columns

                // A narrow column's name may run into the gap after it.
                Text {
                    width: view.widthOf(modelData) + view.cellGap - Theme.dp(4)
                    text: modelData.label
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(17)
                    elide: Text.ElideRight
                }
            }
        }
    }

    Rectangle {
        id: rule
        anchors.top: head.bottom
        anchors.topMargin: Theme.dp(12)
        width: parent.width
        height: 1
        color: Theme.surfaceBorder
    }

    Text {
        anchors.top: rule.bottom
        anchors.topMargin: Theme.dp(60)
        anchors.horizontalCenter: parent.horizontalCenter
        visible: view.rows.length === 0 && !view.store.busy
        text: "No games yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
    }

    ListView {
        id: list

        anchors.top: rule.bottom
        anchors.topMargin: Theme.dp(6)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true
        interactive: false
        model: view.rows
        currentIndex: view.row
        preferredHighlightBegin: Theme.dp(14)
        preferredHighlightEnd: height - Theme.dp(14)
        highlightRangeMode: ListView.ApplyRange
        highlightFollowsCurrentItem: true
        highlightMoveDuration: Theme.durView

        delegate: Item {
            readonly property var game: modelData
            readonly property bool onRow: index === view.row

            width: list.width
            height: view.rowHeight

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: view.cellGap

                Text {
                    width: view.titleWidth
                    anchors.verticalCenter: parent.verticalCenter
                    text: game.title
                    color: onRow && view.activeFocus && !view.onButton ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: onRow && view.activeFocus && !view.onButton ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(22)
                    elide: Text.ElideRight
                }

                Repeater {
                    model: game.slots

                    Item {
                        id: cell

                        readonly property var slot: modelData
                        readonly property bool focused: onRow && index === view.col && view.activeFocus && !view.onButton

                        width: view.widthOf(slot)
                        height: view.thumbHeight
                        scale: focused ? 1.04 : 1.0

                        Behavior on scale { Ease { easing.type: Easing.OutQuint } }

                        Loader {
                            anchors.fill: parent
                            active: cell.focused
                            sourceComponent: FocusRing { cornerRadius: Theme.dp(8) }
                        }

                        ArtFrame {
                            anchors.fill: parent
                            radius: Theme.dp(8)
                            row: cell.slot
                            badge: cell.slot.kind === "picked"
                            badgeLabel: "Pick"
                            badgeMargin: Theme.dp(6)
                            emptyText: "missing"
                            emptySize: Theme.dp(15)
                        }
                    }
                }
            }
        }
    }
}
