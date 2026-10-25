import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "mediaPage"
    focus: true

    signal chromeRequested
    signal detailRequested(var game)
    signal recordingsRequested(var game, string session)
    signal journalRequested(var game, string session)

    readonly property var store: api.screens.media
    readonly property var currentGame: current ? api.allGames.byId(current.gameId) : null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 24
    readonly property real scrimTop: 0.62
    readonly property real scrimMid: 0.78
    readonly property real scrimBottom: 0.95
    readonly property bool modal: menu.open || lightbox
    property bool lightbox: false

    readonly property var rows: store.rows
    readonly property var filters: [
        {
            kind: "shot",
            label: "Screenshots"
        },
        {
            kind: "recording",
            label: "Recordings"
        },
        {
            kind: "journal",
            label: "Journal"
        }
    ]
    property var kinds: ["shot", "recording", "journal"]
    property int filterIndex: 0
    readonly property bool filtering: filterRow.activeFocus
    readonly property var shown: rows.filter(function (r) {
        return kinds.indexOf(r.kind) >= 0;
    })
    property int index: 0
    readonly property var current: index >= 0 && index < shown.length ? shown[index] : null
    readonly property var shotRows: shown.filter(function (r) {
        return r.kind === "shot";
    })
    readonly property int shotIndex: current ? keyIndex(shotRows, current.key) : -1

    readonly property var hints: menu.open ? menu.hints : filtering ? [
        {
            glyph: "A",
            label: kinds.indexOf(filters[filterIndex].kind) >= 0 ? "Hide" : "Show",
            dim: kinds.length === 1 && kinds[0] === filters[filterIndex].kind
        },
        {
            glyph: "B",
            label: "Back"
        }
    ] : lightbox ? [
        {
            glyph: "dpad",
            label: "Previous / next"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ] : [
        {
            glyph: "A",
            label: openLabel,
            dim: current === null
        },
        {
            glyph: "X",
            label: "Game page",
            dim: currentGame === null
        }
    ].concat(current && current.hasJournal && current.kind !== "journal" ? [
        {
            glyph: "Y",
            label: "Journal"
        }
    ] : []).concat([
        {
            glyph: "Start",
            label: "More",
            dim: current === null
        },
        {
            glyph: "B",
            label: "Back"
        }
    ])
    readonly property string openLabel: current ? (current.kind === "shot" ? "View" : current.kind === "recording" ? "Play" : "Read") : "Open"

    readonly property int columns: 4
    readonly property real gap: Theme.dp(26)
    readonly property real sideMargin: Theme.dp(80)
    readonly property real cellWidth: (width - sideMargin * 2 + gap) / columns
    readonly property real cellHeight: (cellWidth - gap) * 9 / 16 + gap
    // Room inside the grid's clip for the focused card's ring and halo, which reach past its cell.
    readonly property real inset: Theme.dp(12)

    Component.onCompleted: store.load()
    onShownChanged: {
        if (index >= shown.length)
            index = Math.max(0, shown.length - 1);
    }

    // The rows reach QML as copies, a new one each read: a row is found again by its key, never by identity.
    function keyIndex(list, key) {
        return list.findIndex(function (r) {
            return r.key === key;
        });
    }

    function leave() {
        lightbox = false;
    }

    function step(d) {
        index = Sound.stepped(index, d, shown.length);
    }

    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.floor(grid.height / cellHeight), shown.length);
    }

    function stepRow(d) {
        var next = index + d * columns;
        if (next < 0 || next >= shown.length) {
            if (d < 0) {
                Sound.tick();
                filterRow.forceActiveFocus();
            } else
                Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function open() {
        if (!current || !currentGame) {
            Sound.edge();
            return;
        }
        if (current.kind === "shot") {
            Sound.enter();
            lightbox = true;
        } else if (current.kind === "recording") {
            page.recordingsRequested(currentGame, current.session);
        } else {
            page.journalRequested(currentGame, current.session);
        }
    }

    // One kind at least stays on: the grid is never filtered down to nothing to show.
    function toggle(kind) {
        var at = kinds.indexOf(kind);
        if (at >= 0 && kinds.length === 1) {
            Sound.edge();
            return;
        }
        Sound.enter();
        var next = kinds.slice();
        if (at >= 0)
            next.splice(at, 1);
        else
            next.push(kind);
        kinds = next;
    }

    function openJournal() {
        if (current && currentGame && current.hasJournal)
            page.journalRequested(currentGame, current.session);
        else
            Sound.edge();
    }

    // The card itself is the copy the menu keeps lit: the cell around it is the grid's transparent ground.
    function cellAnchor() {
        return grid.currentItem ? grid.currentItem.card : grid;
    }

    function cellRect() {
        var card = cellAnchor();
        return Qt.rect(0, 0, card.width, card.height);
    }

    function openMenu() {
        if (!current || !grid.currentItem) {
            Sound.edge();
            return;
        }
        Sound.panel();
        var items = [
            {
                icon: current.kind === "recording" ? "play" : current.kind === "journal" ? "book" : "image",
                label: openLabel,
                action: "open"
            },
            {
                icon: "info",
                label: "Details",
                action: "details"
            }
        ];
        if (current.hasJournal && current.kind !== "journal")
            items.push({
                icon: "book",
                label: "Journal entry",
                action: "journal"
            });
        items.push({
            icon: "trash",
            label: current.kind === "shot" ? "Remove screenshot…" : current.kind === "recording" ? "Remove recording…" : "Remove entry…",
            action: "remove",
            danger: true,
            gap: true
        });
        menu.show(items, cellAnchor(), cellRect(), "", menuAction);
    }

    function hasPair(row) {
        if (row.kind === "recording")
            return row.hasJournal;
        return row.kind === "journal" && rows.some(function (r) {
            return r.kind === "recording" && r.gameId === row.gameId && r.session === row.session;
        });
    }

    function confirmRemove() {
        var kind = current.kind;
        var items = [
            {
                icon: "",
                label: "Keep it",
                action: ""
            },
            {
                icon: "trash",
                label: kind === "shot" ? "Trash the screenshot" : kind === "recording" ? "Trash the recording" : "Trash the entry",
                action: "remove!",
                danger: true
            }
        ];
        if (hasPair(current))
            items.push({
                icon: "trash",
                label: kind === "recording" ? "Trash it and its journal entry" : "Trash it and its recording",
                action: "remove-both!",
                danger: true
            });
        menu.show(items, cellAnchor(), cellRect(), kind === "shot" ? "Remove this screenshot?" : kind === "recording" ? "Remove this recording?" : "Remove this entry?", menuAction);
    }

    function remove(both) {
        var row = current;
        if (row.kind === "shot")
            api.screens.shots.remove(row.gameId, row.name);
        if ((row.kind === "recording" || both) && !api.screens.recordings.remove(row.gameId, row.session))
            return;
        if (row.kind === "journal" || both)
            api.screens.journal.remove(row.gameId, row.session);
    }

    function menuAction(action) {
        if (action === "open") {
            open();
        } else if (action === "details") {
            page.detailRequested(currentGame);
        } else if (action === "journal") {
            openJournal();
        } else if (action === "remove") {
            Sound.panel();
            confirmRemove();
        } else if (action === "remove!" || action === "remove-both!") {
            Sound.enter();
            remove(action === "remove-both!");
        }
        // A request opened a sub-page, which took the focus; only an action that stays here gives it back to the grid.
        if (action === "" || action === "remove!" || action === "remove-both!" || (action === "open" && current && current.kind === "shot"))
            grid.forceActiveFocus();
    }

    Text {
        id: titleText

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        text: page.current ? page.current.gameTitle : "Media"
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(58)
        elide: Text.ElideRight
    }

    Row {
        id: filterRow

        objectName: "mediaFilters"
        anchors.top: titleText.bottom
        anchors.topMargin: Theme.dp(18)
        anchors.left: titleText.left
        spacing: Theme.dp(14)

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat && event.key !== Qt.Key_Left && event.key !== Qt.Key_Right)
                return;
            event.accepted = true;
            if (api.keys.isAccept(event)) {
                page.toggle(page.filters[page.filterIndex].kind);
            } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
                page.filterIndex = Sound.stepped(page.filterIndex, event.key === Qt.Key_Left ? -1 : 1, page.filters.length);
            } else if (event.key === Qt.Key_Down) {
                Sound.tick();
                grid.forceActiveFocus();
            } else if (event.key === Qt.Key_Up) {
                page.chromeRequested();
            } else {
                event.accepted = false;
            }
        }

        Repeater {
            model: page.filters

            Rectangle {
                id: chip

                readonly property bool on: page.kinds.indexOf(modelData.kind) >= 0
                readonly property bool focused: filterRow.activeFocus && index === page.filterIndex

                objectName: "mediaFilter"
                height: Theme.dp(44)
                width: chipContent.width + Theme.dp(36)
                radius: height / 2
                color: on ? Theme.surface : "transparent"
                border.width: Math.max(1, Theme.dp(2))
                border.color: Theme.surfaceBorder
                scale: focused ? 1.04 : 1.0

                Behavior on scale {
                    Ease {
                        duration: Theme.durQuick
                    }
                }

                Pointer {
                    current: chip.focused
                    radius: chip.radius
                    onPicked: {
                        page.filterIndex = index;
                        filterRow.forceActiveFocus();
                    }
                }

                Loader {
                    anchors.fill: parent
                    active: chip.focused
                    sourceComponent: FocusRing {
                        cornerRadius: chip.radius
                        gapWidth: Theme.dp(Theme.ringGap)
                    }
                }

                Row {
                    id: chipContent

                    anchors.centerIn: parent
                    spacing: Theme.dp(8)

                    MenuGlyph {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(18)
                        height: width
                        visible: chip.on
                        kind: "check"
                        tint: Theme.text
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.label
                        color: chip.on ? Theme.text : Theme.textMuted
                        font.family: Theme.sans
                        font.weight: Font.Medium
                        font.pixelSize: Theme.dp(20)
                    }
                }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        visible: page.shown.length === 0 && !page.store.loading
        text: page.store.count === 0 ? "Screenshots, recordings and journal entries land here as you play." : "Nothing of these kinds yet."
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    GridView {
        id: grid

        Scroller {
            rangeBegin: Theme.dp(40)
            rangeEnd: grid.height - Theme.dp(40)
        }

        anchors.top: filterRow.bottom
        anchors.topMargin: Theme.dp(26) - page.inset
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin - page.gap / 2 - page.inset
        anchors.rightMargin: page.sideMargin - page.gap / 2 - page.inset
        topMargin: page.inset
        bottomMargin: page.inset
        leftMargin: page.inset
        rightMargin: page.inset
        clip: true
        focus: true
        model: page.shown
        cellWidth: page.cellWidth
        cellHeight: page.cellHeight
        currentIndex: page.index
        interactive: false
        highlightFollowsCurrentItem: false

        delegate: Item {
            width: grid.cellWidth
            height: grid.cellHeight

            readonly property bool current: index === page.index
            readonly property Item card: cardItem

            Pointer {
                current: parent.current && grid.activeFocus
                radius: Theme.dp(12)
                onPicked: {
                    page.index = index;
                    grid.forceActiveFocus();
                }
            }

            MediaCard {
                id: cardItem
                anchors.fill: parent
                anchors.margins: page.gap / 2
                // `version` is read so the card repaints when its thumbnail lands.
                source: (api.screens.thumbs.version, modelData.image || api.screens.thumbs.url(modelData.thumb))
                kind: modelData.kind
                heading: modelData.kind === "journal" ? modelData.title : ""
                excerpt: modelData.excerpt
                durationText: modelData.durationText
                focused: parent.current && grid.activeFocus && !page.lightbox
                dimmed: !parent.current && grid.activeFocus && !page.lightbox
            }
        }

        Keys.onPressed: function (event) {
            var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
            var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
            var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
            if (event.isAutoRepeat && !arrow && !vertical && !screen)
                return;
            event.accepted = true;
            if (page.lightbox) {
                if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                    Sound.cancel();
                    page.lightbox = false;
                } else if (arrow) {
                    var shots = page.shotRows;
                    var next = Sound.stepped(page.shotIndex, event.key === Qt.Key_Left ? -1 : 1, shots.length);
                    page.index = page.keyIndex(page.shown, shots[next].key);
                }
                return;
            }
            if (api.keys.isAccept(event))
                page.open();
            else if (api.keys.isDetails(event))
                page.currentGame ? page.detailRequested(page.currentGame) : Sound.edge();
            else if (api.keys.isFilters(event))
                page.openJournal();
            else if (api.keys.isMenu(event))
                page.openMenu();
            else if (arrow)
                page.step(event.key === Qt.Key_Left ? -1 : 1);
            else if (vertical)
                page.stepRow(event.key === Qt.Key_Up ? -1 : 1);
            else if (screen)
                page.stepScreen(screen);
            else if (api.keys.isFirst(event) || api.keys.isLast(event))
                page.index = Sound.stepped(page.index, api.keys.isFirst(event) ? -page.shown.length : page.shown.length, page.shown.length);
            else
                event.accepted = false;
        }
    }

    Lightbox {
        anchors.fill: parent
        z: 4
        images: page.shotRows.map(function (r) {
            return r.url;
        })
        index: Math.max(0, page.shotIndex)
        open: page.lightbox
    }

    ActionMenu {
        id: menu

        objectName: "mediaMenu"
        anchors.fill: parent
        z: 5
        copyMargin: Theme.dp(18)

        onDismissed: grid.forceActiveFocus()
    }
}
