import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "dataPage"
    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property var store: api.screens.gameData

    signal closeRequested

    // The store's headings become the sidebar's sections, the rows under each one card.
    readonly property var layout: {
        var rows = [], groups = [];
        store.rows.forEach(function (r) {
            if (r.heading) {
                groups.push({
                    title: r.label,
                    meta: r.display,
                    rows: []
                });
                return;
            }
            rows.push(Object.assign({}, r, {
                section: groups.length > 0 ? groups[groups.length - 1].title : ""
            }));
            groups[groups.length - 1].rows.push(rows.length - 1);
        });
        return {
            rows: rows,
            groups: groups
        };
    }
    readonly property var icons: ({
            "Saves": "download",
            "Backups": "refresh",
            "Wine Prefix": "sliders",
            "Storage": "folder"
        })
    readonly property var sections: layout.groups.map(function (g) {
        return {
            name: g.title,
            icon: page.icons[g.title] || "folder"
        };
    })

    readonly property var row: body.currentRow
    readonly property bool inRows: body.inRows

    readonly property var hints: editor.open ? editor.hints : dialog.open ? dialog.hints : [
        {
            glyph: "A",
            label: inRows ? "Select" : "Open",
            dim: inRows && (!row || row.type !== "action" || row.disabled === true)
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property real sideMargin: Theme.dp(90)

    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (!args.game)
            return;
        body.section = 0;
        body.zone = "side";
        store.load(args.game.id);
    }

    function activate(index, row) {
        if (row.type !== "action" || row.disabled === true) {
            Sound.edge();
            return;
        }
        if (row.key === "run") {
            Sound.panel();
            editor.edit({
                key: "exe",
                type: "path",
                label: "Run a Program in the Prefix",
                value: "~"
            }, function (path) {
                if (path !== "")
                    store.runProgram(path) ? Sound.enter() : Sound.edge();
            });
            return;
        }
        var ask = store.question(row.key);
        if (!ask) {
            store.act(row.key) ? Sound.enter() : Sound.edge();
            return;
        }
        dialog.ask({
            message: ask.title,
            detail: ask.detail,
            yes: ask.confirm,
            index: 0
        }, function (yes) {
            if (yes)
                store.act(row.key);
            body.cards.forceActiveFocus();
        });
    }

    Connections {
        target: page.store
        function onFinished(key, ok, message) {
            if (ok)
                Notices.show(message);
            else
                Notices.fail(message);
        }
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: page.store.total !== "" ? "DATA · " + page.store.total : "DATA"
    }

    Text {
        anchors.centerIn: parent
        visible: page.store.count === 0
        text: page.store.loading ? "Reading the game's folders…" : page.store.error
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    CardSections {
        id: body

        anchors.fill: parent
        focus: true
        visible: page.store.count > 0
        columnsTop: header.y + header.height + Theme.dp(32)
        floor: hintBar.y
        sideMargin: page.sideMargin
        rows: page.layout.rows
        groups: page.layout.groups
        sections: page.sections
        dimmed: editor.open || dialog.open

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onCancelled: page.closeRequested()
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        hints: page.hints
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: body.cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onClosed: body.cards.forceActiveFocus()
    }

    ConfirmDialog {
        id: dialog

        anchors.fill: parent
        z: 4
    }
}
