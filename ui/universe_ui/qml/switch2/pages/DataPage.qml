import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core" as Base

FocusScope {
    id: page

    objectName: "dataPage"

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var store: api.screens.gameData

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "OK"
        }
    ]

    signal closeRequested

    focus: true

    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args && args.gameId)
            store.load(args.gameId);
    }

    function activate(index, row) {
        Sound.play("ok");
        if (row.key === "run") {
            shell.browse({
                title: "Run a Program in the Prefix",
                path: "~",
                files: true
            }, function (path) {
                if (path !== null)
                    store.runProgram(path);
            });
            return;
        }
        var ask = store.question(row.key);
        if (!ask) {
            store.act(row.key);
            return;
        }
        shell.dialogAsk({
            message: ask.title,
            detail: ask.detail,
            buttons: ["Cancel", ask.confirm],
            danger: ask.danger ? 1 : -1
        }, function (i) {
            if (i === 1)
                store.act(row.key);
        });
    }

    Connections {
        target: page.store
        function onFinished(key, ok, message) {
            if (ok)
                Base.Notices.show(message);
            else
                Base.Notices.fail(message);
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Data Management"
        subtitle: page.game ? page.game.title : ""
        trailing: page.store.total
    }

    Label {
        anchors.centerIn: list
        visible: page.store.count === 0
        text: page.store.loading ? "Reading the game's folders…" : page.store.error
        color: Theme.textMuted
    }

    SettingsRows {
        id: list

        x: Theme.dp(530)
        y: Theme.dp(226)
        width: Theme.dp(1150)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        shell: page.shell
        model: page.store.rows

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
    }
}
