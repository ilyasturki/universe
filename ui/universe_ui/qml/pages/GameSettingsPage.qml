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
        : [ { glyph: "A", label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle" : "Change" },
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
            picker.show(cards, opts, Math.max(0, row.choices.indexOf(row.value)));
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

    // The game's art behind the top of the page, settling into the ground. Faded as one
    // layer: item opacity would thin the gradient too and let the art's edge through.
    Item {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: Theme.dp(560)
        opacity: 0.55
        layer.enabled: true

        BackgroundStage {
            anchors.fill: parent
            game: page.game
            blurRadius: 30
            zoomEnabled: false
            overscan: 1.06
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.30) }
                GradientStop { position: 0.45; color: Qt.rgba(0.055, 0.059, 0.075, 0.70) }
                GradientStop { position: 0.75; color: Qt.rgba(0.055, 0.059, 0.075, 0.94) }
                GradientStop { position: 1.00; color: Theme.ground }
            }
        }
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: Theme.dp(88)

        CoverCard {
            id: tile
            width: Theme.dp(88)
            height: width
            game: page.game
            cornerRadius: Theme.dp(14)
            selected: true
            selectedScale: 1.0
            ringOpacity: 0
            showHeart: false
        }

        Column {
            anchors.left: tile.right
            anchors.leftMargin: Theme.dp(28)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            CapsLabel {
                text: "GAME SETTINGS"
            }

            Row {
                width: parent.width
                spacing: Theme.dp(20)

                Text {
                    id: title
                    text: page.game ? page.game.title : ""
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Bold
                    font.pixelSize: Theme.dp(42)
                    elide: Text.ElideRight
                    width: Math.min(implicitWidth, parent.width - meta.width - parent.spacing)
                }

                GameMetaLine {
                    id: meta
                    anchors.bottom: title.bottom
                    anchors.bottomMargin: Theme.dp(6)
                    game: page.game
                    showYear: false
                }
            }
        }
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
        dimmed: picker.open || sheet.open

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

    ChipPicker {
        id: picker

        property int pendingIndex: -1

        // Drops from the focused row, its right edge on the row's value.
        x: cards.x + cards.focusRect.x + cards.focusRect.width - Theme.dp(16) - width
        y: Math.min(hintBar.y - height - Theme.dp(20),
                    cards.y + cards.focusRect.y + cards.focusRect.height + Theme.dp(8))
        z: 2

        onChosen: function(index) {
            var choices = page.form.row(pendingIndex).choices || [];
            picker.hide();
            cards.forceActiveFocus();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                page.form.setValue(pendingIndex, choices[index]);
            }
        }
        onDismissed: {
            picker.hide();
            cards.forceActiveFocus();
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

        onAccepted: function(value) {
            page.form.setValue(pendingIndex, value);
            cards.forceActiveFocus();
        }
        onDismissed: cards.forceActiveFocus()
    }
}
