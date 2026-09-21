import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    signal chromeRequested
    signal detailRequested(var game)
    signal screenshotsRequested(var game, string name)
    signal recordingsRequested(var game, string session)
    signal journalRequested(var game, string session)

    readonly property var store: api.screens.media
    readonly property var currentGame: current ? api.allGames.byId(current.gameId) : null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 24
    readonly property real scrimTop: 0.62
    readonly property real scrimMid: 0.78
    readonly property real scrimBottom: 0.95
    readonly property bool modal: picker.open || menu.open || lightbox
    property bool lightbox: false

    readonly property var kinds: ["All", "Screenshots", "Recordings", "Journal"]
    readonly property var kindKeys: ["", "shot", "recording", "journal"]
    property int kindIndex: 0
    property string gameFilter: ""

    readonly property var games: {
        var seen = {}, out = [], all = store.rows;
        for (var i = 0; i < all.length; i++) {
            var r = all[i];
            if (!seen[r.gameId]) {
                seen[r.gameId] = true;
                out.push({
                    id: r.gameId,
                    title: r.gameTitle
                });
            }
        }
        out.sort(function (a, b) {
            return a.title.localeCompare(b.title);
        });
        return out;
    }
    readonly property var rows: {
        var all = store.rows;
        return all.filter(function (r) {
            return (kindKeys[kindIndex] === "" || r.kind === kindKeys[kindIndex]) && (gameFilter === "" || r.gameId === gameFilter);
        });
    }
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property var shotRows: rows.filter(function (r) {
        return r.kind === "shot";
    })
    readonly property int shotIndex: current ? shotRows.indexOf(current) : -1

    readonly property var hints: picker.open ? picker.hints : menu.open ? menu.hints : lightbox ? [
        {
            glyph: "dpad",
            label: "Previous / next"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ] : chipBar.activeFocus ? [
        {
            glyph: "A",
            label: "Change"
        },
        {
            glyph: "B",
            label: "Back to grid"
        }
    ] : [
        {
            glyph: "A",
            label: current ? (current.kind === "shot" ? "View" : current.kind === "recording" ? "Play" : "Read") : "Open",
            dim: current === null
        },
        {
            glyph: "X",
            label: "Game details",
            dim: current === null
        },
        {
            glyph: "Y",
            label: "Journal entry",
            dim: !(current && current.hasJournal && current.kind !== "journal")
        },
        {
            glyph: "Start",
            label: "More",
            dim: current === null
        },
        {
            glyph: "LT RT",
            label: "Kind"
        }
    ]

    readonly property int columns: 4
    readonly property real gap: Theme.dp(26)
    readonly property real sideMargin: Theme.dp(80)
    readonly property real cellWidth: (width - sideMargin * 2 + gap) / columns
    readonly property real cellHeight: (cellWidth - gap) * 9 / 16 + gap + Theme.dp(58)

    Component.onCompleted: {
        store.load();
        if (api.memory.has("mediaKind"))
            kindIndex = Math.min(api.memory.get("mediaKind"), kinds.length - 1);
    }
    onKindIndexChanged: {
        api.memory.set("mediaKind", kindIndex);
        index = 0;
        grid.contentY = 0;
    }
    onGameFilterChanged: {
        index = 0;
        grid.contentY = 0;
    }
    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
    }

    function leave() {
        picker.hide();
        lightbox = false;
    }

    function cycleKind(d) {
        kindIndex = (kindIndex + d + kinds.length) % kinds.length;
        Sound.collection();
    }

    function gameOptions() {
        var all = store.rows;
        var out = [
            {
                label: "All games",
                trailing: all.length.toString()
            }
        ];
        for (var i = 0; i < games.length; i++) {
            var id = games[i].id;
            out.push({
                label: games[i].title,
                trailing: all.filter(function (r) {
                    return r.gameId === id;
                }).length.toString()
            });
        }
        return out;
    }

    function gameFilterIndex() {
        return games.findIndex(function (g) {
            return g.id === gameFilter;
        }) + 1;
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
            if (d < 0) {
                Sound.panel();
                chipBar.forceActiveFocus();
            } else {
                Sound.edge();
            }
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
                label: current.kind === "shot" ? "View" : current.kind === "recording" ? "Play" : "Read",
                action: "open"
            },
            {
                icon: "info",
                label: "Game details",
                action: "details"
            }
        ];
        if (current.kind === "shot")
            items.push({
                icon: "camera",
                label: "All of this game's",
                action: "shots"
            });
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
                danger: true
            });
        menu.show(items, cellAnchor(), cellRect(), current.gameTitle + "  ·  " + current.dateText, menuAction);
    }

    function menuAction(action) {
        if (action === "open") {
            open();
        } else if (action === "details") {
            page.detailRequested(currentGame);
        } else if (action === "shots") {
            page.screenshotsRequested(currentGame, current.name);
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

    Item {
        id: header

        z: 2
        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: titleText.height + Theme.dp(14) + metaText.height

        Text {
            id: titleText
            anchors.left: parent.left
            anchors.right: chipBar.left
            anchors.rightMargin: Theme.dp(40)
            text: page.current ? page.current.gameTitle : "Media"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(58)
            elide: Text.ElideRight
        }

        Text {
            id: metaText
            anchors.top: titleText.bottom
            anchors.topMargin: Theme.dp(14)
            anchors.left: parent.left
            text: page.current ? (page.current.kind === "shot" ? "SCREENSHOT" : page.current.kind === "recording" ? "RECORDING  ·  " + page.current.title : "JOURNAL  ·  " + page.current.title) + "  ·  " + page.current.dateText : page.store.count === 0 && !page.store.loading ? "Screenshots, recordings and journal entries land here as you play." : ""
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(20)
            font.letterSpacing: 1.5
            elide: Text.ElideRight
            width: parent.width - chipBar.width - Theme.dp(40)
        }

        FocusScope {
            id: chipBar

            anchors.right: parent.right
            anchors.bottom: parent.bottom
            width: chips.width
            height: chips.height

            property int index: 0

            function step(d) {
                index = (index + d + 2) % 2;
                Sound.tick();
            }

            function pointTo(i) {
                index = i;
                chipBar.forceActiveFocus();
            }

            function openPicker() {
                Sound.panel();
                if (index === 0)
                    picker.show(kindChip, page.kinds.map(function (k) {
                        return {
                            label: k
                        };
                    }), page.kindIndex);
                else
                    picker.show(gameChip, page.gameOptions(), page.gameFilterIndex());
            }

            Row {
                id: chips
                spacing: Theme.dp(14)

                Chip {
                    id: kindChip
                    label: page.kinds[page.kindIndex]
                    trailing: page.rows.length.toString()
                    focused: chipBar.activeFocus && chipBar.index === 0
                    onPicked: chipBar.pointTo(0)
                }

                Chip {
                    id: gameChip
                    label: page.gameFilter === "" ? "All games" : (page.games.find(function (g) {
                            return g.id === page.gameFilter;
                        }) || {
                            title: ""
                        }).title
                    focused: chipBar.activeFocus && chipBar.index === 1
                    onPicked: chipBar.pointTo(1)
                }
            }

            ChipPicker {
                id: picker

                onChosen: function (index) {
                    picker.hide();
                    chipBar.forceActiveFocus();
                    if (chipBar.index === 0) {
                        page.kindIndex = index;
                        Sound.collection();
                    } else {
                        page.gameFilter = index === 0 ? "" : page.games[index - 1].id;
                        Sound.sort();
                    }
                }
                onDismissed: {
                    picker.hide();
                    chipBar.forceActiveFocus();
                }
            }

            Keys.onLeftPressed: chipBar.step(-1)
            Keys.onRightPressed: chipBar.step(1)
            Keys.onUpPressed: page.chromeRequested()
            Keys.onDownPressed: function (event) {
                Sound.panel();
                grid.forceActiveFocus();
            }

            Keys.onPressed: function (event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    chipBar.openPicker();
                } else if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    grid.forceActiveFocus();
                }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0 && page.store.count > 0
        text: "Nothing of that kind" + (page.gameFilter !== "" ? " for this game" : "") + "."
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    GridView {
        id: grid

        Wheel {}

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(34)
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin - page.gap / 2
        anchors.rightMargin: page.sideMargin - page.gap / 2
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
                caption: modelData.gameTitle
                subcaption: modelData.dateText
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
            else if (api.keys.isPageUp(event) || api.keys.isPageDown(event));else
                event.accepted = false;
        }

        Keys.onReleased: function (event) {
            if (event.isAutoRepeat)
                return;
            var d = api.keys.isPageUp(event) ? -1 : api.keys.isPageDown(event) ? 1 : 0;
            if (!d)
                return;
            event.accepted = true;
            page.cycleKind(d);
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
