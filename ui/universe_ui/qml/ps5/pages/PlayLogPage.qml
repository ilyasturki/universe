import QtQuick
import "../core"
import "../sound"
import "../ui"

// Every session of one game, newest first: when, how long, how it ended; A reads what the game wrote.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var store: api.screens.sessions
    readonly property var rows: store.rows
    readonly property var hints: []
    readonly property bool strip: false
    // A session to land on, as the dock's "sessions" asks.
    readonly property string landing: args && args.session ? String(args.session) : ""

    signal closeRequested

    focus: true

    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args && args.gameId)
            store.load(args.gameId);
        Qt.callLater(landOnSession);
    }

    onRowsChanged: Qt.callLater(landOnSession)

    function landOnSession() {
        if (landing === "")
            return;
        var i = entries.findIndex(function (r) {
            return r.session === page.landing;
        });
        if (i >= 0)
            list.index = i;
    }

    // Never played: one still row says so, where the sessions will be.
    readonly property var entries: rows.length === 0 ? [
        {
            key: "none",
            label: "Not played yet",
            type: "static",
            icon: "clock",
            display: "",
            detail: "Every session and what the game wrote will be listed here."
        }
    ] : rows.map(function (r) {
        var bits = [];
        if (!r.live) {
            bits.push(r.endText);
            if (r.hasRecording)
                bits.push("Recorded");
            if (r.hasJournal)
                bits.push("Journal");
        }
        return {
            key: r.live ? "live" : r.session,
            label: r.live ? "Playing now" : r.dateText,
            type: "action",
            icon: r.live ? "play" : r.bad ? "warning" : "clock",
            display: r.live ? "" : r.durationText,
            detail: bits.join(" · "),
            session: r.session,
            live: r.live,
            hasRecording: r.hasRecording,
            hasJournal: r.hasJournal
        };
    })

    function read(row) {
        Sound.play("ok");
        shell.push("pages/SessionLogPage.qml", {
            gameId: game ? game.id : "",
            session: row.session,
            label: row.label
        });
    }

    function options() {
        var row = list.currentRow;
        if (!row || row.session === undefined) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var items = [
            {
                label: "Read the Log",
                glyph: "terminal",
                act: "log"
            }
        ];
        if (row.hasRecording)
            items.push({
                label: "Watch the Recording",
                glyph: "film",
                act: "watch"
            });
        if (row.hasJournal)
            items.push({
                label: "Journal Entry",
                glyph: "journal",
                act: "journal"
            });
        var id = game ? game.id : "";
        shell.menu(row.label, items, function (act) {
            if (act === "log") {
                page.read(row);
            } else if (act === "watch") {
                api.screens.album.load(id);
                page.shell.push("pages/PlayerPage.qml", {
                    gameId: id,
                    session: row.session
                });
            } else if (act === "journal") {
                api.screens.news.load(id);
                page.shell.push("pages/ArticlePage.qml", {
                    gameId: id,
                    session: row.session
                });
            }
        });
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: "Play Log"
        trailing: page.rows.length > 0 ? page.rows.length + (page.rows.length === 1 ? " session" : " sessions") : ""
    }

    SettingsRows {
        id: list

        shell: page.shell
        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(20)
        width: parent.width - x - Theme.dp(Theme.columnRight + 16)
        height: parent.height - y - Theme.dp(40)
        focus: true
        model: page.entries

        onActivated: function (index, row) {
            page.read(row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")
    }
}
