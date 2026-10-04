import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Feed.js" as Feed
import "../ui/Removal.js" as Removal

// The console's Media Gallery: every capture and journal entry, newest first, in tabs; a shot opens full screen,
// a recording in the player, an entry in the journal.
FocusScope {
    id: page

    objectName: "mediaGalleryPage"
    property var shell: null
    property var args: ({})
    readonly property var hints: []
    readonly property bool strip: false
    signal closeRequested
    focus: true

    readonly property var store: api.screens.media
    readonly property var rows: store.rows
    readonly property var tabs: ["All", "Screenshots", "Recordings", "Journal"]
    readonly property var kinds: ["", "shot", "recording", "journal"]
    property int tab: 0
    property string filterId: ""
    property bool oldestFirst: false
    // "tabs", "rail" or "grid"
    property string zone: "grid"
    property int tabCursor: 0
    property bool viewing: false
    property int shotIndex: 0
    // What args named to open: { path, session }, held until the rows hold it.
    property var landing: null

    readonly property var shown: {
        var kind = kinds[tab];
        var out = rows.filter(function (r) {
            return (filterId === "" || r.gameId === filterId) && (kind === "" || r.kind === kind);
        });
        return oldestFirst ? out.slice().reverse() : out;
    }
    readonly property int index: grid.index
    readonly property var current: grid.index >= 0 && grid.index < shown.length ? shown[grid.index] : null
    readonly property var shots: shown.filter(function (r) {
        return r.kind === "shot";
    })
    readonly property var game: filterId !== "" ? api.allGames.byId(filterId) : null
    readonly property var channels: Feed.channels(rows)
    readonly property bool modal: viewing

    readonly property real columnsWidth: width - Theme.dp(Theme.edge) - Theme.dp(Theme.columnRight)
    readonly property real cellWidth: Math.floor((columnsWidth - Theme.dp(32) * 2) / 3)
    readonly property real cellHeight: Math.round(cellWidth * 282 / 504)

    Component.onCompleted: store.load()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
        if (args.path || args.session)
            landing = {
                path: args.path || "",
                session: args.session || ""
            };
        Qt.callLater(land);
    }

    onRowsChanged: Qt.callLater(land)

    // A capture named by args: its row focused, then opened as A would.
    function land() {
        if (!landing || store.loading)
            return;
        var want = landing;
        var at = -1;
        if (want.path !== "")
            at = shown.findIndex(function (r) {
                return r.kind === "shot" && r.path === want.path;
            });
        if (at < 0 && want.session !== "")
            at = shown.findIndex(function (r) {
                return r.kind === "recording" && r.session === want.session;
            });
        if (at < 0 && rows.length === 0)
            return;
        landing = null;
        if (at < 0)
            return;
        zone = "grid";
        grid.index = at;
        grid.forceActiveFocus();
        open();
    }

    function kindName(row) {
        return row.kind === "shot" ? "Screenshot" : row.kind === "recording" ? "Recording" : "Entry";
    }

    function open() {
        var row = current;
        if (!row) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (row.kind === "shot") {
            shotIndex = Math.max(0, shots.indexOf(row));
            viewing = true;
        } else if (row.kind === "recording") {
            shell.push("pages/PlayerPage.qml", {
                gameId: row.gameId,
                session: row.session
            });
        } else {
            shell.push("pages/ArticlePage.qml", {
                gameId: row.gameId,
                session: row.session
            });
        }
    }

    function openJournal() {
        var row = current;
        if (!row || !(row.hasJournal || row.kind === "journal")) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        viewing = false;
        shell.push("pages/ArticlePage.qml", {
            gameId: row.gameId,
            session: row.session
        });
    }

    function openGame() {
        var row = current;
        if (!row || !shell.openGame || !api.allGames.byId(row.gameId)) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.openGame(row.gameId);
    }

    // The row as Removal.js reads it: a journal row from the timeline says whether a recording shares its session.
    function removalRow(row) {
        if (row.kind !== "journal")
            return row;
        return Object.assign({
            state: "written",
            hasRecording: rows.some(function (r) {
                return r.kind === "recording" && r.gameId === row.gameId && r.session === row.session;
            })
        }, row);
    }

    function remove(row) {
        var done = function () {
            viewing = false;
            store.load();
        };
        if (row.kind === "shot")
            Removal.screenshot(shell, api.screens, row, done);
        else if (row.kind === "recording")
            Removal.recording(shell, api.screens, row, done);
        else
            Removal.entry(shell, api.screens, removalRow(row), done);
    }

    function options() {
        var row = current;
        if (!row) {
            Sound.play("edge");
            return;
        }
        var items = [
            {
                label: row.kind === "shot" ? "View" : row.kind === "recording" ? "Play" : "Read",
                glyph: row.kind === "shot" ? "image" : row.kind === "recording" ? "play" : "journal",
                act: "open"
            }
        ];
        if (row.hasJournal && row.kind !== "journal")
            items.push({
                label: "Journal Entry",
                glyph: "journal",
                act: "journal"
            });
        if (shell.openGame && api.allGames.byId(row.gameId))
            items.push({
                label: "Go to Game",
                glyph: "gamepad",
                act: "game"
            });
        items.push({
            label: filterId === "" ? "Only " + row.gameTitle : "All Games",
            glyph: "filter",
            act: "filter"
        }, {
            label: "Delete " + kindName(row) + "…",
            glyph: "trash",
            act: "remove",
            gap: true
        });
        shell.showMenu({
            title: row.gameTitle + " · " + row.dateText,
            items: items
        }, function (i) {
            if (i < 0)
                return;
            var act = items[i].act;
            if (act === "open")
                open();
            else if (act === "journal")
                openJournal();
            else if (act === "game")
                openGame();
            else if (act === "filter")
                setFilter(filterId === "" ? row.gameId : "");
            else
                remove(row);
        });
    }

    function viewerOptions() {
        var row = shots[shotIndex];
        if (!row) {
            Sound.play("edge");
            return;
        }
        var items = [];
        if (row.hasJournal)
            items.push({
                label: "Journal Entry",
                glyph: "journal",
                act: "journal"
            });
        items.push({
            label: "Delete Screenshot…",
            glyph: "trash",
            act: "remove"
        });
        shell.showMenu({
            title: row.gameTitle + " · " + row.dateText,
            items: items
        }, function (i) {
            if (i < 0)
                return;
            if (items[i].act === "journal")
                openJournal();
            else
                remove(row);
        });
    }

    function setFilter(id) {
        filterId = id;
        grid.index = 0;
    }

    function switchTab(t) {
        if (t < 0 || t >= tabs.length) {
            Sound.play("edge");
            return;
        }
        if (t === tab)
            return;
        Sound.play("tick");
        tab = t;
        tabCursor = t;
        grid.index = 0;
        gridIn.restart();
    }

    function railAction(id) {
        if (id === "sort") {
            shell.pick({
                title: "Sort by",
                choices: ["Date Added (New - Old)", "Date Added (Old - New)"],
                index: oldestFirst ? 1 : 0,
                at: {
                    x: rail.x + rail.width + Theme.dp(24),
                    y: rail.y
                }
            }, function (i) {
                if (i >= 0 && (i === 1) !== oldestFirst) {
                    oldestFirst = i === 1;
                    grid.index = 0;
                }
            });
        } else if (id === "filter") {
            var ids = [""].concat(channels.map(function (c) {
                return c.id;
            }));
            shell.pick({
                title: "Show",
                choices: ["All Games"].concat(channels.map(function (c) {
                    return c.title + "  (" + c.count + ")";
                })),
                index: Math.max(0, ids.indexOf(filterId)),
                at: {
                    x: rail.x + rail.width + Theme.dp(24),
                    y: rail.y + Theme.dp(104)
                }
            }, function (i) {
                if (i >= 0)
                    setFilter(ids[i]);
            });
        }
    }

    function stepShot(d) {
        var next = Sound.stepped(shotIndex, d, shots.length);
        if (next === shotIndex)
            return;
        shotIndex = next;
        grid.index = shown.indexOf(shots[next]);
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (viewing) {
            event.accepted = true;
            if (arrow) {
                stepShot(event.key === Qt.Key_Left ? -1 : 1);
                return;
            }
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.play("back");
                viewing = false;
            } else if (api.keys.isMenu(event))
                viewerOptions();
            else if (api.keys.isFilters(event))
                openJournal();
            return;
        }
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            if (!event.isAutoRepeat)
                switchTab(tab + (api.keys.isPrevPage(event) ? -1 : 1));
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (zone === "tabs") {
            if (arrow) {
                event.accepted = true;
                tabCursor = Sound.stepped(tabCursor, event.key === Qt.Key_Left ? -1 : 1, tabs.length);
                switchTab(tabCursor);
            } else if (event.key === Qt.Key_Down || api.keys.isAccept(event)) {
                event.accepted = true;
                Sound.play("tick");
                zone = "grid";
                grid.forceActiveFocus();
            } else if (event.key === Qt.Key_Up) {
                event.accepted = true;
                Sound.play("edge");
            }
            return;
        }
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            openJournal();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            openGame();
        } else if (api.keys.isMenu(event) && zone === "rail") {
            event.accepted = true;
            Sound.play("edge");
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Media Gallery"
        game: page.game
    }

    GalleryTabs {
        x: Theme.dp(143)
        y: Theme.dp(145)
        labels: page.tabs
        current: page.tab
        index: page.tabCursor
        active: page.zone === "tabs" && page.activeFocus
        onPointed: function (i) {
            page.zone = "tabs";
            page.tabCursor = i;
            page.switchTab(i);
            page.forceActiveFocus();
        }
    }

    Label {
        x: Theme.dp(Theme.edge)
        y: Theme.dp(295) - height / 2
        text: page.tabs[page.tab] + ": " + page.shown.length
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(26)
    }

    Label {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.columnRight)
        y: Theme.dp(295) - height / 2
        text: "Sort by: Date Added (" + (page.oldestFirst ? "Old - New" : "New - Old") + ")"
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(26)
    }

    GalleryRail {
        id: rail
        z: 2
        x: Theme.dp(76)
        y: Theme.dp(336)
        focus: page.zone === "rail" && !page.viewing
        items: [
            {
                id: "sort",
                glyph: "sort",
                label: "Sort"
            },
            {
                id: "filter",
                glyph: "filter",
                label: page.filterId === "" ? "Filter" : "Filter · " + (page.game ? page.game.title : page.filterId)
            }
        ]
        onActivated: function (id) {
            page.railAction(id);
        }
        onPointed: page.zone = "rail"
        onEscapedRight: {
            if (page.shown.length > 0) {
                page.zone = "grid";
                grid.forceActiveFocus();
            } else
                Sound.play("edge");
        }
    }

    GalleryGrid {
        id: grid

        x: Theme.dp(Theme.edge)
        y: Theme.dp(336)
        width: page.columnsWidth
        height: parent.height - y
        focus: page.zone === "grid" && !page.viewing
        model: page.shown
        cellWidth: page.cellWidth
        cellHeight: page.cellHeight
        gapX: Theme.dp(32)
        gapY: Theme.dp(34)

        transform: Translate {
            id: shift
        }

        onEscapedLeft: {
            page.zone = "rail";
            rail.forceActiveFocus();
        }
        onEscapedUp: {
            page.zone = "tabs";
            page.tabCursor = page.tab;
            page.forceActiveFocus();
        }
        onPointed: page.zone = "grid"
        onActivated: page.open()
        onOptionsRequested: page.options()
    }

    ParallelAnimation {
        id: gridIn
        NumberAnimation {
            target: grid
            property: "opacity"
            from: 0
            to: 1
            duration: Theme.durTab
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: shift
            property: "x"
            from: Theme.dp(40)
            to: 0
            duration: Theme.durTab
            easing.type: Easing.OutCubic
        }
    }

    Label {
        x: grid.x
        y: grid.y + Theme.dp(60)
        width: grid.width
        visible: page.shown.length === 0 && !page.store.loading
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: page.rows.length === 0 ? "Screenshots, recordings and journal entries land here as you play." : "Nothing here for this tab."
        color: Theme.textMuted
    }

    Rectangle {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(46)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(26)
        width: tabHint.implicitWidth + Theme.dp(28)
        height: Theme.dp(52)
        radius: Theme.dp(6)
        color: Qt.rgba(0, 0, 0, 0.45)
        visible: !page.viewing

        Row {
            id: tabHint
            anchors.centerIn: parent
            spacing: Theme.dp(8)

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "LB"
                unit: Theme.dp(30)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: "/"
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "RB"
                unit: Theme.dp(30)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                leftPadding: Theme.dp(6)
                text: "Switch Tabs"
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    GalleryViewer {
        anchors.fill: parent
        z: 5
        images: page.shots.map(function (r) {
            return r.url;
        })
        index: page.shotIndex
        open: page.viewing
        caption: page.viewing && page.shots[page.shotIndex] ? page.shots[page.shotIndex].gameTitle + "  ·  " + page.shots[page.shotIndex].dateText : ""
    }
}
