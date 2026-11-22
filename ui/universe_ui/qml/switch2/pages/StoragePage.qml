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

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: list.currentRow && list.currentRow.key.indexOf("trash:") === 0 ? "Move to Trash" : "OK",
            dim: !list.currentRow || list.currentRow.type !== "action"
        }
    ]

    readonly property var entries: (store.error !== "" ? [
            {
                key: "retry",
                label: "Measure Again",
                type: "action",
                detail: store.error
            }
        ] : store.loading && store.count === 0 ? [
            {
                key: "measuring",
                label: "Measuring every folder…",
                type: "static"
            }
        ] : []).concat(store.rows)
    onEntriesChanged: list.swap(entries, "retry")

    signal closeRequested

    focus: true

    Component.onCompleted: store.load()

    // Back from a game's Data page: its sizes may have moved.
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
            danger: 1
        }, function (i) {
            if (i === 1)
                store.act(row.key);
        });
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Data Management"
        subtitle: page.store.free !== "" ? page.store.free + " free where games install" : ""
        trailing: page.store.used !== "" ? page.store.used + " used" : ""
    }

    SettingsRows {
        id: list

        x: Theme.dp(530)
        y: Theme.dp(226)
        width: Theme.dp(1150)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        shell: page.shell
        Component.onCompleted: swap(page.entries, "retry")

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
    }
}
