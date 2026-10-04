import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Feed.js" as Feed
import "../ui/Removal.js" as Removal

// The Journal: every session's entry, newest first, one card each; A reads it, or asks for it when none was written.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var hints: []
    readonly property bool strip: false
    signal closeRequested
    focus: true

    readonly property var store: api.screens.news
    readonly property var rows: store.rows
    property string filterId: ""
    // "rail" or "list"
    property string zone: "list"
    property int index: 0
    property double now: Date.now()

    readonly property var entries: rows.filter(function (r) {
        return filterId === "" || r.gameId === filterId;
    })
    readonly property var current: index >= 0 && index < entries.length ? entries[index] : null
    readonly property var channels: Feed.channels(rows)
    readonly property var game: filterId !== "" ? api.allGames.byId(filterId) : null
    readonly property bool anyPending: rows.some(function (r) {
        return r.state === "pending";
    })

    readonly property real cardHeight: Theme.dp(176)
    readonly property real gap: Theme.dp(20)

    Component.onCompleted: store.loadAll()
    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    onEntriesChanged: {
        if (index >= entries.length)
            index = Math.max(0, entries.length - 1);
    }
    onIndexChanged: Theme.reveal(list, index * (cardHeight + gap), index * (cardHeight + gap) + cardHeight + Theme.dp(20), list.height)

    Timer {
        interval: 1000
        running: page.anyPending
        repeat: true
        onTriggered: page.now = Date.now()
    }

    function blank(row) {
        return row.state === "none" || row.state === "deferred" || row.state === "failed";
    }

    function activate() {
        var row = current;
        if (!row || row.state === "pending") {
            Sound.play("edge");
            return;
        }
        if (blank(row)) {
            write(row, false);
            return;
        }
        Sound.play("ok");
        shell.push("pages/ArticlePage.qml", {
            gameId: row.gameId,
            session: row.session
        });
    }

    // Hands the session to the journal module: a first entry, another try, or a fresh one over what is there.
    function write(row, again) {
        if (!row || row.state === "pending") {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        store.write(row.gameId, row.session, again);
    }

    function watch(row) {
        if (!row || !row.hasRecording) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/PlayerPage.qml", {
            gameId: row.gameId,
            session: row.session
        });
    }

    function pickFilter() {
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
                y: rail.y
            }
        }, function (i) {
            if (i >= 0) {
                filterId = ids[i];
                index = 0;
            }
        });
    }

    function options() {
        var row = current;
        if (!row) {
            Sound.play("edge");
            return;
        }
        var pending = row.state === "pending";
        var items = [];
        if (row.state === "written")
            items.push({
                label: "Read",
                glyph: "journal",
                act: "read"
            });
        if (blank(row))
            items.push({
                label: row.state === "none" ? "Write the Entry" : "Try Again Now",
                glyph: row.state === "none" ? "plus" : "refresh",
                act: "write"
            });
        if (row.state === "written")
            items.push({
                label: "Write It Again",
                glyph: "refresh",
                act: "rewrite"
            });
        if (row.hasRecording)
            items.push({
                label: "Watch the Recording",
                glyph: "film",
                act: "recording"
            });
        items.push({
            label: filterId === "" ? "Only " + row.gameTitle : "All Games",
            glyph: "filter",
            act: "filter"
        });
        if (row.state !== "none" || row.hasRecording)
            items.push({
                label: pending ? "Cancel the Writing…" : row.state === "none" ? "Delete the Recording…" : "Delete Entry…",
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
            if (act === "read")
                activate();
            else if (act === "write")
                write(row, false);
            else if (act === "rewrite")
                write(row, true);
            else if (act === "recording")
                watch(row);
            else if (act === "filter") {
                filterId = filterId === "" ? row.gameId : "";
                index = 0;
            } else if (row.state === "none")
                Removal.recording(shell, api.screens, Object.assign({
                    hasJournal: false
                }, row), function () {
                    store.loadAll();
                });
            else
                Removal.entry(shell, api.screens, row, function () {
                    store.loadAll();
                });
        });
    }

    function step(d) {
        index = Sound.stepped(index, d, entries.length);
    }

    Keys.onPressed: function (event) {
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !vertical && !screen)
            return;
        if (zone === "rail")
            return;
        if (vertical) {
            event.accepted = true;
            step(event.key === Qt.Key_Up ? -1 : 1);
        } else if (screen) {
            event.accepted = true;
            index = Sound.paged(index, screen, 1, Math.max(1, Math.floor(list.height / (cardHeight + gap))), entries.length);
        } else if (event.key === Qt.Key_Left) {
            event.accepted = true;
            Sound.play("tick");
            zone = "rail";
            rail.forceActiveFocus();
        } else if (event.key === Qt.Key_Right) {
            event.accepted = true;
            Sound.play("edge");
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            activate();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            watch(current);
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            Sound.play("ok");
            pickFilter();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Journal"
        game: page.game
    }

    Label {
        x: Theme.dp(Theme.edge)
        y: Theme.dp(214) - height / 2
        text: "Entries: " + page.entries.length
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(26)
    }

    Label {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.columnRight)
        y: Theme.dp(214) - height / 2
        text: "Newest first"
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(26)
    }

    GalleryRail {
        id: rail
        z: 2
        x: Theme.dp(76)
        y: Theme.dp(262)
        focus: page.zone === "rail"
        items: [
            {
                id: "filter",
                glyph: "filter",
                label: page.filterId === "" ? "Filter" : "Filter · " + (page.game ? page.game.title : page.filterId)
            }
        ]
        onActivated: function (id) {
            page.pickFilter();
        }
        onPointed: page.zone = "rail"
        onEscapedRight: {
            page.zone = "list";
            page.forceActiveFocus();
        }
    }

    Flickable {
        id: list

        x: Theme.dp(Theme.edge) - Theme.dp(10)
        y: Theme.dp(252)
        width: parent.width - Theme.dp(Theme.edge) - Theme.dp(Theme.columnRight) + Theme.dp(20)
        height: parent.height - y
        contentWidth: width
        contentHeight: Math.max(0, page.entries.length * (page.cardHeight + page.gap)) + Theme.dp(60)
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Repeater {
            model: page.entries

            JournalCard {
                x: Theme.dp(10)
                y: Theme.dp(10) + index * (page.cardHeight + page.gap)
                width: list.width - Theme.dp(20)
                height: page.cardHeight
                row: modelData
                now: page.now
                focused: page.zone === "list" && page.activeFocus && index === page.index
                onPicked: {
                    Sound.play("tick");
                    page.zone = "list";
                    page.index = index;
                    page.forceActiveFocus();
                }
            }
        }
    }

    Swipe {
        flickable: list
        ease: scrollEase
    }

    Scrollbar {
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(20)
        anchors.top: list.top
        anchors.bottom: list.bottom
        flickable: list
    }

    Label {
        x: Theme.dp(Theme.edge)
        y: Theme.dp(330)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        visible: page.entries.length === 0
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: page.rows.length === 0 ? "No entries yet. The journal writes one after each session." : "No entries for this game yet."
        color: Theme.textMuted
    }
}
