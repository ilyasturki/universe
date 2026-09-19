import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: view

    readonly property var store: api.screens.artworkOverview
    readonly property var rows: store.rows
    readonly property var columns: store.columns

    signal openRequested(var game, string slot)
    signal escapedLeft()
    signal escapedUp()
    signal message(string text)

    property int row: 0
    property int col: 0
    readonly property var currentRow: row >= 0 && row < rows.length ? rows[row] : null
    readonly property var currentColumn: col >= 0 && col < columns.length ? columns[col] : null
    readonly property var job: store.job
    readonly property bool fetching: job !== null && job !== undefined && (job.ok === null || job.ok === undefined)

    readonly property real titleWidth: Theme.dp(300)
    readonly property real thumbHeight: Theme.dp(96)
    readonly property real rowHeight: Theme.dp(110)
    readonly property real cellGap: Theme.dp(18)

    readonly property var hints: [
        { glyph: "A", label: currentRow && currentColumn ? "Open " + currentColumn.label.toLowerCase() : "Open", dim: currentRow === null },
        { glyph: "X", label: fetching ? "Fetching" + (job.total > 0 ? " " + job.done + "/" + job.total : "") + "…" : "Fetch missing art", dim: fetching },
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
        } else if (api.keys.isDetails(event)) {
            if (fetching) {
                Sound.edge();
            } else {
                Sound.enter();
                store.refreshAll();
            }
        } else if (event.key === Qt.Key_Up) {
            row === 0 ? view.escapedUp() : move(-1, 0);
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

    Row {
        id: head

        x: view.titleWidth + view.cellGap
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
                    color: onRow && view.activeFocus ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: onRow && view.activeFocus ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(22)
                    elide: Text.ElideRight
                }

                Repeater {
                    model: game.slots

                    Item {
                        id: cell

                        readonly property var slot: modelData
                        readonly property bool focused: onRow && index === view.col && view.activeFocus

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
