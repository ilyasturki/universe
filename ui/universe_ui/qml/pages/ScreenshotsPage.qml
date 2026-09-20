import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "screenshotsPage"
    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string landing: args.name || ""
    readonly property string landingSession: args.session || ""
    readonly property var store: api.screens.shots
    readonly property var rows: grid.ordered
    readonly property var playing: api.universe.currentSession
    // The running session's shots come first, under their own heading, while this game is the one playing.
    readonly property string since: game && playing && playing.session_id !== undefined && playing.id === game.id ? String(playing.started_at || "") : ""
    property alias index: grid.index
    readonly property int mine: grid.mine
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

    readonly property real sideMargin: Theme.dp(90)

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
        if (!rows)
            return;
        var i = rows.findIndex(function(r) { return r.name === landing; });
        if (i < 0 && landingSession !== "")
            i = rows.findIndex(function(r) { return r.session === landingSession; });
        if (i >= 0)
            index = i;
    }

    function step(d) {
        index = Sound.stepped(index, d, rows.length);
    }

    function stepScreen(d) {
        index = Sound.paged(index, d, columns, Math.floor(grid.height / cellHeight), rows.length);
    }

    function stepRow(d) {
        if (!grid.stepLine(d))
            Sound.edge();
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
        return grid.currentCard();
    }

    function cellRect() {
        var card = cellAnchor();
        return Qt.rect(0, 0, card.width, card.height);
    }

    function openMenu() {
        if (!current) {
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
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !arrow && !vertical && !screen)
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
        else if (screen)
            stepScreen(screen);
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
        height: title.height

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
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No screenshots yet — X in the dock, or the pad's screenshot button."
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ShotGrid {
        id: grid

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(34)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        anchors.left: parent.left
        anchors.right: parent.right
        topPadding: 0
        sideMargin: page.sideMargin
        rows: page.store.rows
        since: page.since
        active: page.activeFocus && !page.lightbox
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
