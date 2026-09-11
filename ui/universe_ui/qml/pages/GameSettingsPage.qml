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

    readonly property var hints: sheet.open ? sheet.hints
        : picker.open ? picker.hints
        : [ { glyph: "A", label: list.currentRow && list.currentRow.type === "bool" ? "Toggle" : "Change" },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)

    onGameChanged: if (game) form.load(game.id)

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else if (row.type === "enum") {
            Sound.panel();
            var opts = row.choices.map(function(c) { return { label: c }; });
            picker.pendingIndex = index;
            picker.show(list, opts, Math.max(0, row.choices.indexOf(row.value)));
        } else {
            Sound.panel();
            sheet.pendingIndex = index;
            sheet.show(row.label, row.value, row.type === "path");
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: title.height + Theme.dp(10) + subtitle.height

        Text {
            id: title
            text: "Settings"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(46)
        }

        Text {
            id: subtitle
            anchors.top: title.bottom
            anchors.topMargin: Theme.dp(10)
            text: page.game ? page.game.title : ""
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
        }
    }

    SettingsList {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(20)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin - Theme.dp(22)
        anchors.rightMargin: page.sideMargin - Theme.dp(22)
        focus: true
        rows: page.form.rows
        dimmed: picker.open || sheet.open

        onActivated: page.activate(index, row)
        onEscapedUp: Sound.edge()

        Keys.onPressed: {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.closeRequested();
            }
        }
    }

    ChipPicker {
        id: picker

        property int pendingIndex: -1

        // Drops beside the focused row rather than under a chip.
        x: page.width / 2 - width / 2
        y: Math.min(page.height - height - Theme.dp(120),
                    list.y + Theme.dp(60) + Math.max(0, (list.index - 1)) * (list.rowHeight + Theme.dp(4)))
        z: 2

        onChosen: {
            var choices = page.form.row(pendingIndex).choices || [];
            picker.hide();
            list.forceActiveFocus();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                page.form.setValue(pendingIndex, choices[index]);
            }
        }
        onDismissed: {
            picker.hide();
            list.forceActiveFocus();
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    KeyboardSheet {
        id: sheet

        property int pendingIndex: -1

        anchors.fill: parent
        z: 3

        onAccepted: {
            page.form.setValue(pendingIndex, value);
            list.forceActiveFocus();
        }
        onDismissed: list.forceActiveFocus()
    }
}
