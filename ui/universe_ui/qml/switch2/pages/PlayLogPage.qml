import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var store: api.screens.sessions
    readonly property var rows: store.rows

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "Read the log"
        }
    ]

    signal closeRequested

    focus: true

    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args && args.gameId)
            store.load(args.gameId);
    }

    readonly property var entries: rows.map(function (r) {
        return {
            key: r.live ? "live" : r.session,
            label: r.live ? "Playing now" : r.dateText,
            type: "action",
            detail: r.live ? "" : r.durationText + " · " + r.endText + (r.hasRecording ? " · Recorded" : ""),
            session: r.session
        };
    })

    function activate(index, row) {
        Sound.play("ok");
        shell.push("pages/SessionLogPage.qml", {
            gameId: game ? game.id : "",
            session: row.session,
            label: row.label
        });
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Play Log"
        subtitle: page.game ? page.game.title : ""
        trailing: page.rows.length > 0 ? page.rows.length + (page.rows.length === 1 ? " session" : " sessions") : ""
    }

    Label {
        anchors.centerIn: list
        visible: page.rows.length === 0
        text: "Not played yet. Every session and what the software wrote will be listed here."
        color: Theme.textMuted
    }

    SettingsRows {
        id: list

        x: Theme.dp(530)
        y: Theme.dp(226)
        width: Theme.dp(1150)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        model: page.entries

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
    }
}
