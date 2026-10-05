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
    readonly property var hints: []
    readonly property bool strip: false

    readonly property var icons: ({
            saves_status: "file",
            saves_folder: "folder",
            cloud_status: "cloud",
            keep_local: "display",
            keep_cloud: "cloud",
            backup: "download",
            restore: "restart",
            export: "folder",
            prefix: "cube",
            move: "folder",
            winecfg: "sliders",
            winetricks: "puzzle",
            run: "play",
            kill: "stop",
            reset: "trash",
            install: "storage",
            universe: "image",
            recordings: "film",
            logs: "terminal"
        })

    readonly property var entries: store.rows.map(function (r) {
        return Object.assign({}, r, {
            icon: r.heading ? "" : r.key.indexOf("restore:") === 0 ? "clock" : page.icons[r.key] || ""
        });
    })

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
            index: 0,
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

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: "Saved Data and Storage"
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

        shell: page.shell
        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(20)
        width: parent.width - x - Theme.dp(Theme.columnRight + 16)
        height: parent.height - y - Theme.dp(40)
        focus: true
        model: page.entries

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")
    }
}
