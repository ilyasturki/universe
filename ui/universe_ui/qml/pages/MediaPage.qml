import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

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
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property var shotRows: rows.filter(function (r) {
        return r.kind === "shot";
    })
    readonly property int shotIndex: current ? shotRows.indexOf(current) : -1

    readonly property var hints: menu.open ? menu.hints : lightbox ? [
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
            glyph: "Start",
            label: "More",
            dim: current === null
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]
    readonly property string openLabel: current ? (current.kind === "shot" ? "View" : current.kind === "recording" ? "Play" : "Read") : "Open"

    readonly property int columns: 4
    readonly property real gap: Theme.dp(26)
    readonly property real sideMargin: Theme.dp(80)
    readonly property real cellWidth: (width - sideMargin * 2 + gap) / columns
    readonly property real cellHeight: (cellWidth - gap) * 9 / 16 + gap
    // Room inside the grid's clip for the focused card's ring and halo, which reach past its cell.
    readonly property real inset: Theme.dp(12)

    Component.onCompleted: store.load()
    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
    }

    function leave() {
        lightbox = false;
    }

    function step(d) {
        index = Sound.stepped(index, d, rows.length);
    }

    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.floor(grid.height / cellHeight), rows.length);
    }

    function stepRow(d) {
        var next = index + d * columns;
        if (next < 0 || next >= rows.length) {
            if (d < 0)
                page.chromeRequested();
            else
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
        if (current.kind === "shot")
            items.push({
                icon: "trash",
                label: "Remove screenshot…",
                action: "remove",
                danger: true,
                gap: true
            });
        menu.show(items, cellAnchor(), cellRect(), "", menuAction);
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
            menu.show([
                {
                    icon: "",
                    label: "Keep it",
                    action: ""
                },
                {
                    icon: "trash",
                    label: "Trash the screenshot",
                    action: "remove!",
                    danger: true
                }
            ], cellAnchor(), cellRect(), "Remove this screenshot?", menuAction);
        } else if (action === "remove!") {
            Sound.enter();
            api.screens.shots.remove(current.gameId, current.name);
        }
        // A request opened a sub-page, which took the focus; only an action that stays here gives it back to the grid.
        if (action === "" || action === "remove!" || (action === "open" && current && current.kind === "shot"))
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

    Text {
        anchors.centerIn: parent
        visible: page.store.count === 0 && !page.store.loading
        text: "Screenshots, recordings and journal entries land here as you play."
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    GridView {
        id: grid

        Wheel {}

        anchors.top: titleText.bottom
        anchors.topMargin: Theme.dp(34) - page.inset
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
        model: page.rows
        cellWidth: page.cellWidth
        cellHeight: page.cellHeight
        currentIndex: page.index
        interactive: false
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: Theme.dp(40)
        preferredHighlightEnd: height - Theme.dp(40)
        highlightRangeMode: GridView.ApplyRange
        highlightMoveDuration: Theme.durView

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
                    page.index = page.rows.indexOf(shots[next]);
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
                page.index = Sound.stepped(page.index, api.keys.isFirst(event) ? -page.rows.length : page.rows.length, page.rows.length);
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

        anchors.fill: parent
        z: 5
        copyMargin: Theme.dp(18)

        onDismissed: grid.forceActiveFocus()
    }
}
