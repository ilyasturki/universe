import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string landing: args.name || ""
    readonly property string landingSession: args.session || ""
    readonly property var store: api.screens.shots
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property string currentSession: current ? current.session : ""
    property bool lightbox: false

    signal closeRequested()
    signal jumpRequested(string source, string session)

    readonly property var hints: menu.open ? menu.hints
        : lightbox ? [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ]
        : [ { glyph: "A", label: "View", dim: current === null },
            { glyph: "Y", label: "Journal entry", dim: !(current && current.hasJournal) },
            { glyph: "Start", label: "More", dim: current === null },
            { glyph: "B", label: "Back" } ]

    readonly property int columns: 4
    readonly property real gap: Theme.dp(24)
    readonly property real sideMargin: Theme.dp(90)
    readonly property real cellWidth: (width - sideMargin * 2 + gap) / columns
    readonly property real cellHeight: (cellWidth - gap) * 9 / 16 + gap

    onGameChanged: {
        index = 0;
        lightbox = false;
        if (game)
            store.load(game.id);
        land();
    }
    onLandingChanged: land()
    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
        land();
    }

    function land() {
        var i = rows.findIndex(function(r) { return r.name === landing; });
        if (i < 0 && landingSession !== "")
            i = rows.findIndex(function(r) { return r.session === landingSession; });
        if (i >= 0)
            index = i;
    }

    function step(d) {
        index = Sound.stepped(index, d, rows.length);
    }

    function stepRow(d) {
        var next = index + d * columns;
        if (next < 0 || next >= rows.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function view() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.enter();
        lightbox = true;
    }

    function openJournal() {
        if (current && current.hasJournal)
            page.jumpRequested("pages/JournalPage.qml", current.session);
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
        var items = [ { icon: "image", label: "View", action: "view" } ];
        if (current.hasJournal)
            items.push({ icon: "book", label: "Journal entry", action: "journal" });
        items.push({ icon: "trash", label: "Remove screenshot…", action: "remove", danger: true });
        menu.show(items, cellAnchor(), cellRect(), current.dateText, menuAction);
    }

    function menuAction(action) {
        if (action === "view") {
            view();
        } else if (action === "journal") {
            openJournal();
        } else if (action === "remove") {
            Sound.panel();
            menu.show([ { icon: "", label: "Keep it", action: "" },
                        { icon: "trash", label: "Trash the screenshot", action: "remove!", danger: true } ],
                      cellAnchor(), cellRect(), "Remove this screenshot?", menuAction);
        } else if (action === "remove!") {
            Sound.enter();
            store.remove(current.gameId, current.name);
        }
        if (action !== "remove" && action !== "journal")
            page.forceActiveFocus();
    }

    Keys.onPressed: function(event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !arrow && !vertical)
            return;
        event.accepted = true;
        if (lightbox) {
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                lightbox = false;
            } else if (arrow) {
                step(event.key === Qt.Key_Left ? -1 : 1);
            }
            return;
        }
        if (api.keys.isAccept(event))
            view();
        else if (api.keys.isCancel(event))
            page.closeRequested();
        else if (api.keys.isMenu(event))
            openMenu();
        else if (api.keys.isFilters(event))
            openJournal();
        else if (arrow)
            step(event.key === Qt.Key_Left ? -1 : 1);
        else if (vertical)
            stepRow(event.key === Qt.Key_Up ? -1 : 1);
        else
            event.accepted = false;
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: title.height + Theme.dp(10) + meta.height

        Text {
            id: title
            anchors.left: parent.left
            anchors.right: parent.right
            text: page.game ? page.game.title : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(46)
            elide: Text.ElideRight
        }

        Text {
            id: meta
            anchors.top: title.bottom
            anchors.topMargin: Theme.dp(10)
            text: "SCREENSHOTS  ·  " + page.rows.length + (page.current ? "  ·  " + page.current.dateText : "")
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(20)
            font.letterSpacing: 1.5
        }
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No screenshots yet — X in the dock, or the pad's screenshot button."
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    GridView {
        id: grid

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(34)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin - page.gap / 2
        anchors.rightMargin: page.sideMargin - page.gap / 2
        clip: true
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

            ShotCard {
                id: cardItem
                anchors.fill: parent
                anchors.margins: page.gap / 2
                source: modelData.url
                caption: modelData.dateText
                focused: parent.current && page.activeFocus && !page.lightbox
                dimmed: !parent.current && page.activeFocus && !page.lightbox
                journaled: modelData.hasJournal
            }
        }
    }

    Lightbox {
        anchors.fill: parent
        z: 4
        images: page.rows.map(function(r) { return r.url; })
        index: page.index
        open: page.lightbox
    }

    HintBar {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
        z: 4
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 5
        copyMargin: Theme.dp(18)

        onDismissed: page.forceActiveFocus()
    }
}
