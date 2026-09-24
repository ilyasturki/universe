import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "gameSettingsPage"
    focus: true

    // { game }: one game's settings; `key` (and a module setting's `settingModule`) lands the cursor on that row,
    // Advanced turned on if the row sits behind it.
    property var args: ({})
    readonly property var game: args.game || null
    readonly property var form: api.screens.gameSettings
    property string landKey: ""
    property string landModule: ""

    signal closeRequested
    signal message(string text)

    // One sidebar entry per card: the game's own, then the modules'; Advanced only adds rows inside them.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        var first = form.rows[g.rows[0]] || {};
        return {
            name: g.title,
            icon: first.module ? "grid" : page.icons[g.title] || "play",
            group: first.module ? "Modules" : ""
        };
    })
    readonly property var icons: ({
            "Display": "screen",
            "Overlay": "gauge",
            "Launch": "sliders",
            "Desktop and library": "heart-outline"
        })

    readonly property var row: body.currentRow
    readonly property bool inRows: body.inRows
    // X on a row: a value of the game's own goes, a variable the game set goes out of its map.
    readonly property string rowAction: row && row.entry ? "Remove" : "Reset"
    readonly property bool canReset: inRows && row !== null && form.resettable(row)

    readonly property var hints: editor.open ? editor.hints : menu.open ? menu.hints : [
        {
            glyph: "A",
            label: !inRows ? "Open" : row && row.type === "bool" ? "Toggle" : row && row.type === "action" ? row.action || "Select" : "Change",
            dim: inRows && (!row || row.disabled === true || row.type === "info")
        }
    ].concat(inRows ? [
        {
            glyph: "Start",
            label: "More",
            dim: moreItems.length === 0
        }
    ] : []).concat([
        {
            glyph: "B",
            label: "Back"
        }
    ])

    // X and Y do these straight away; More lists them.
    readonly property var moreItems: (canReset ? [
            {
                icon: "refresh",
                label: rowAction === "Remove" ? "Remove" : "Reset to default",
                action: "reset"
            }
        ] : []).concat(form.hasAdvanced ? [
        {
            icon: "sliders",
            label: form.showAdvanced ? "Hide advanced" : "Show advanced",
            action: "advanced"
        }
    ] : [])

    readonly property real sideMargin: Theme.dp(90)

    // The derived game is still stale here: read the args themselves. The rows come at once; the cursor settles once
    // the page is laid out (one call however often the args change): a search hit lands, anything else starts on the sidebar.
    onArgsChanged: {
        if (!args.game)
            return;
        landKey = args.key || "";
        landModule = args.settingModule || "";
        body.section = 0;
        body.zone = "side";
        form.load(args.game.id);
        Qt.callLater(page.settle);
    }

    function settle() {
        if (!landNow())
            body.reset();
    }

    // A search hit: the card holding the row, the cursor on it.
    function landNow() {
        if (landKey === "")
            return false;
        var i = form.reveal(landKey, landModule);
        landKey = "";
        if (i < 0)
            return false;
        body.landOn(i);
        return true;
    }

    function toggleAdvanced() {
        Sound.panel();
        form.showAdvanced = !form.showAdvanced;
    }

    function resetRow() {
        if (!canReset) {
            Sound.edge();
            return;
        }
        form.reset(body.cards.index) ? Sound.enter() : Sound.edge();
    }

    function openMenu() {
        if (!inRows || moreItems.length === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        menu.show(moreItems, body.cards, body.cards.focusRect, "", function (action) {
            body.cards.forceActiveFocus();
            if (action === "reset")
                page.resetRow();
            else if (action === "advanced")
                page.toggleAdvanced();
        });
    }

    function activate(index, row) {
        if (row.disabled === true || row.type === "info") {
            Sound.edge();
        } else if (row.map === true) {
            Sound.panel();
            editor.promptPair(row.label.replace(/…$/, ""), row.fields, "", "", function (name, value) {
                form.setMapEntry(index, name, value) ? Sound.enter() : Sound.edge();
            });
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else {
            Sound.panel();
            editor.edit(row, function (value) {
                form.setValue(index, value);
            });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) {
            page.message(text);
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
        label: "GAME SETTINGS"
    }

    CardSections {
        id: body

        anchors.fill: parent
        focus: true
        columnsTop: header.y + header.height + Theme.dp(32)
        floor: hintBar.y
        sideMargin: page.sideMargin
        rows: page.form.rows
        groups: page.groups
        sections: page.sections
        dimmed: editor.open

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

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 4

        onDismissed: body.cards.forceActiveFocus()
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat || editor.open || menu.open)
            return;
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            page.openMenu();
        } else if (api.keys.isFilters(event) && form.hasAdvanced) {
            event.accepted = true;
            page.toggleAdvanced();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            page.resetRow();
        }
    }
}
