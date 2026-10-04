import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core" as Base

FocusScope {
    id: page

    objectName: "storagePage"

    property var shell: null
    property var args: ({})
    readonly property var store: api.screens.storage
    readonly property var hints: []
    readonly property bool strip: false

    readonly property var entries: store.rows.map(function (r) {
        var kind = r.key.split(":")[0];
        return Object.assign({}, r, {
            icon: r.heading ? "" : kind === "root" ? "storage" : kind === "game" ? "gamepad" : kind === "trash" ? "trash" : ""
        });
    })

    signal closeRequested

    focus: true

    Component.onCompleted: store.load()

    // Back from a game's page: its sizes may have moved.
    onActiveFocusChanged: {
        if (activeFocus && store.count > 0)
            store.load();
    }

    function activate(index, row) {
        Sound.play("ok");
        if (row.gameId) {
            shell.push("pages/DataPage.qml", {
                gameId: row.gameId
            });
            return;
        }
        var ask = store.question(row.key);
        if (!ask)
            return;
        shell.dialogAsk({
            message: ask.title,
            detail: row.target,
            buttons: ["Cancel", ask.confirm],
            index: 0,
            danger: 1
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
        title: "Console Storage"
        trailing: page.store.free !== "" ? page.store.free + " free" : ""
    }

    Label {
        anchors.centerIn: list
        visible: page.store.count === 0
        text: page.store.loading ? "Measuring every folder…" : page.store.error
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
