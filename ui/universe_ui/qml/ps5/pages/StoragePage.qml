import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "storagePage"

    property var shell: null
    property var args: ({})
    readonly property var store: api.screens.storage
    readonly property var hints: []
    readonly property bool strip: false

    readonly property var entries: (store.error !== "" ? [
            {
                key: "retry",
                label: "Measure Again",
                type: "action",
                detail: store.error,
                icon: "refresh"
            }
        ] : store.loading && store.count === 0 ? [
            {
                key: "measuring",
                label: "Measuring every folder…",
                type: "static"
            }
        ] : []).concat(rows)
    onEntriesChanged: list.swap(entries, "retry")
    readonly property var rows: store.rows.map(function (r) {
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
        if (row.key === "retry") {
            store.load();
            return;
        }
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

    SettingsRows {
        id: list

        shell: page.shell
        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(20)
        width: parent.width - x - Theme.dp(Theme.columnRight + 16)
        height: parent.height - y - Theme.dp(40)
        focus: true
        Component.onCompleted: swap(page.entries, "retry")

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")
    }
}
