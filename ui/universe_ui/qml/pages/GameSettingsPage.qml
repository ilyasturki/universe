import QtQuick
import "../core"
import "../sound"
import "../ui"

// One game's settings: the core keys Library1.Set takes, then every enabled module's
// game-scope settings from its schema. Rows come built from the host; the type picks the control.
FocusScope {
    id: page

    focus: true

    property var game: null
    readonly property var form: api.screens.gameSettings

    signal closeRequested()

    readonly property var hints: editor.open ? editor.hints
        : [ { glyph: "A", label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle" : "Change" },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)

    onGameChanged: if (game) form.load(game.id)

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else {
            Sound.panel();
            editor.edit(index, row);
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
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

    SettingsCards {
        id: cards

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        focus: true
        compact: true
        rows: page.form.rows
        groups: page.form.groups
        dimmed: editor.open

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedUp: Sound.edge()

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.closeRequested();
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        // Above the sheets, whose panels reach under it.
        z: 3
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onAccepted: function(index, value) { page.form.setValue(index, value); }
        onClosed: cards.forceActiveFocus()
    }
}
