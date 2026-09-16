import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: view

    readonly property var store: api.screens.artworkOverview
    readonly property var tiles: store.tiles
    readonly property var counts: store.counts

    signal openRequested(var game, string slot)
    signal escapedLeft()
    signal escapedUp()

    property string zone: "slots"
    property int slotIndex: 0
    property int filterIndex: 0
    property int gridIndex: 0

    readonly property var slotNames: store.slotNames
    readonly property var filterNames: store.filterNames
    readonly property bool fetching: store.job !== null && store.job !== undefined && (store.job.ok === null || store.job.ok === undefined)
    readonly property int columns: store.slot === "box_front" ? 6 : store.slot === "square" ? 5 : store.slot === "background" ? 3 : 4
    readonly property real cellWidth: Math.floor(width / columns)
    readonly property real artHeight: Math.round((cellWidth - Theme.dp(16)) / store.aspect)

    readonly property var hints: {
        var out = [ { glyph: "A", label: zone === "grid" ? "Open" : "Show" } ];
        if (!fetching)
            out.push({ glyph: "X", label: "Fetch missing art" });
        out.push({ glyph: "dpad", label: "Navigate" });
        out.push({ glyph: "B", label: "Sections" });
        return out;
    }

    function load() {
        store.load();
        slotIndex = Math.max(0, slotNames.findIndex(function(s) { return s.slot === store.slot; }));
        filterIndex = Math.max(0, filterNames.findIndex(function(f) { return f.filter === store.filter; }));
    }

    onTilesChanged: if (gridIndex >= tiles.length) gridIndex = Math.max(0, tiles.length - 1)

    Component.onDestruction: store.unload()

    function chooseSlot(i) {
        slotIndex = i;
        gridIndex = 0;
        store.slot = slotNames[i].slot;
    }

    function chooseFilter(i) {
        filterIndex = i;
        gridIndex = 0;
        store.filter = filterNames[i].filter;
    }

    function activate() {
        if (zone === "grid") {
            var game = tiles.length > 0 ? api.allGames.byId(tiles[gridIndex].id) : null;
            if (!game) {
                Sound.edge();
                return;
            }
            view.openRequested(game, store.slot);
        } else if (zone === "filters") {
            Sound.enter();
            chooseFilter(filterIndex);
        } else {
            Sound.enter();
            chooseSlot(slotIndex);
        }
    }

    function moveChip(d) {
        var slots = zone === "slots";
        var cur = slots ? slotIndex : filterIndex;
        var n = Sound.stepped(cur, d, (slots ? slotNames : filterNames).length);
        if (n !== cur)
            slots ? chooseSlot(n) : chooseFilter(n);
    }

    function moveGrid(d) {
        gridIndex = Sound.stepped(gridIndex, d, tiles.length);
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
            if (zone === "grid") {
                if (gridIndex < columns) {
                    Sound.panel();
                    zone = "filters";
                } else {
                    moveGrid(-columns);
                }
            } else if (zone === "filters") {
                Sound.tick();
                zone = "slots";
            } else {
                view.escapedUp();
            }
        } else if (event.key === Qt.Key_Down) {
            if (zone === "slots") {
                Sound.tick();
                zone = "filters";
            } else if (zone === "filters") {
                if (tiles.length === 0) {
                    Sound.edge();
                } else {
                    Sound.panel();
                    zone = "grid";
                }
            } else {
                moveGrid(columns);
            }
        } else if (event.key === Qt.Key_Left) {
            if (zone === "grid" && gridIndex % columns === 0)
                view.escapedLeft();
            else
                zone === "grid" ? moveGrid(-1) : moveChip(-1);
        } else if (event.key === Qt.Key_Right) {
            zone === "grid" ? moveGrid(1) : moveChip(1);
        } else {
            event.accepted = false;
        }
    }

    Connections {
        target: view.store
        function onMessage(text) { view.message(text); }
    }

    signal message(string text)

    Row {
        id: slotChips
        spacing: Theme.dp(12)

        Repeater {
            model: view.slotNames

            Chip {
                label: modelData.label
                active: index === view.slotIndex
                focused: view.zone === "slots" && index === view.slotIndex && view.activeFocus
            }
        }
    }

    Text {
        anchors.left: slotChips.right
        anchors.leftMargin: Theme.dp(24)
        anchors.right: fetchButton.left
        anchors.rightMargin: Theme.dp(24)
        anchors.verticalCenter: slotChips.verticalCenter
        text: view.store.slotUse
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(19)
        elide: Text.ElideRight
    }

    Row {
        id: fetchButton
        anchors.right: parent.right
        anchors.verticalCenter: slotChips.verticalCenter
        spacing: Theme.dp(10)
        opacity: view.fetching ? 0.5 : 1.0

        ButtonGlyph {
            glyph: "X"
            anchors.verticalCenter: parent.verticalCenter
        }

        Chip {
            label: view.fetching ? "Fetching…" : "Fetch missing art"
            icon: "download"
        }
    }

    Row {
        id: filterChips
        anchors.top: slotChips.bottom
        anchors.topMargin: Theme.dp(14)
        spacing: Theme.dp(12)

        Repeater {
            model: view.filterNames

            Chip {
                label: modelData.label
                badge: String(view.counts[modelData.filter] === undefined ? 0 : view.counts[modelData.filter])
                active: index === view.filterIndex && view.store.filter === modelData.filter
                focused: view.zone === "filters" && index === view.filterIndex && view.activeFocus
            }
        }
    }

    Text {
        anchors.left: filterChips.right
        anchors.leftMargin: Theme.dp(24)
        anchors.right: parent.right
        anchors.verticalCenter: filterChips.verticalCenter
        visible: text !== ""
        text: {
            var job = view.store.job;
            if (job && (job.ok === null || job.ok === undefined))
                return (job.total > 0 ? job.done + "/" + job.total + "  " : "") + job.message;
            return view.store.busy ? "reading…" : "";
        }
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(19)
        elide: Text.ElideRight
    }

    Text {
        anchors.top: filterChips.bottom
        anchors.topMargin: Theme.dp(60)
        anchors.horizontalCenter: parent.horizontalCenter
        visible: view.tiles.length === 0 && !view.store.busy
        text: view.store.filter === "all" ? "No games yet." : "No game has a " + view.store.filter + " " + view.store.slotLabel.toLowerCase() + "."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
    }

    GridView {
        id: grid

        anchors.top: filterChips.bottom
        anchors.topMargin: Theme.dp(26)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true
        interactive: false
        model: view.tiles
        cellWidth: view.cellWidth
        cellHeight: view.artHeight + Theme.dp(58)
        currentIndex: view.zone === "grid" ? view.gridIndex : -1
        preferredHighlightBegin: Theme.dp(20)
        preferredHighlightEnd: height - Theme.dp(20)
        highlightRangeMode: GridView.ApplyRange
        highlightFollowsCurrentItem: true
        opacity: view.zone === "grid" || !view.activeFocus ? 1.0 : 0.8

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        delegate: Item {
            readonly property bool focused: view.zone === "grid" && index === view.gridIndex && view.activeFocus

            width: grid.cellWidth
            height: grid.cellHeight

            Item {
                id: art
                x: Theme.dp(8)
                y: Theme.dp(8)
                width: parent.width - Theme.dp(16)
                height: view.artHeight
                scale: focused ? 1.04 : 1.0

                Behavior on scale { Ease { easing.type: Easing.OutQuint } }

                Loader {
                    anchors.fill: parent
                    active: focused
                    sourceComponent: FocusRing { cornerRadius: Theme.dp(10) }
                }

                RoundedMask {
                    anchors.fill: parent
                    radius: Theme.dp(10)

                    Rectangle {
                        anchors.fill: parent
                        color: modelData.kind === "missing" ? Qt.rgba(0.88, 0.40, 0.35, 0.10) : Theme.surface
                        border.width: modelData.kind === "missing" ? 1 : 0
                        border.color: Qt.rgba(0.88, 0.40, 0.35, 0.5)
                    }

                    Image {
                        anchors.fill: parent
                        source: modelData.url
                        fillMode: view.store.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                        asynchronous: true
                        sourceSize.width: 480
                    }

                    Text {
                        anchors.centerIn: parent
                        visible: modelData.url === ""
                        text: "missing"
                        color: Theme.textMuted
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(17)
                    }

                    Rectangle {
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: Theme.dp(8)
                        width: Theme.dp(12)
                        height: width
                        radius: width / 2
                        color: modelData.kind === "picked" ? "#5fd48a" : modelData.kind === "default" ? "#7fb2ff" : "#e0655a"
                        border.width: 1
                        border.color: Qt.rgba(0, 0, 0, 0.5)
                    }
                }
            }

            Text {
                anchors.top: art.bottom
                anchors.topMargin: Theme.dp(10)
                x: art.x
                width: art.width
                text: modelData.title
                color: focused ? Theme.text : Theme.textSecondary
                font.family: Theme.sans
                font.weight: focused ? Font.DemiBold : Font.Medium
                font.pixelSize: Theme.dp(18)
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }
}
