import QtQuick
import "../core"
import "../sound"
import "../ui"

// One module's settings: its switch, then its global settings once it runs. Rows come built
// from the host; the type picks the control.
FocusScope {
    id: page

    focus: true

    property string module: ""
    readonly property var form: api.screens.module
    readonly property var info: form.info

    signal closeRequested()

    readonly property var hints: editor.open ? editor.hints
        : [ { glyph: "A", label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle" : "Change",
              dim: !cards.currentRow || cards.currentRow.disabled === true },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)

    onModuleChanged: if (module !== "") form.load(module)

    function activate(index, row) {
        if (row.disabled === true) {
            Sound.edge();
        } else if (row.type === "bool") {
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

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: Theme.dp(88)

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            CapsLabel {
                text: "MODULE"
            }

            Text {
                width: parent.width
                text: page.info.name || ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: (page.info.meta || "")
                      + (page.info.warning
                         ? (page.info.meta ? " · " : "") + "<font color=\"#e0655a\">" + page.info.warning + "</font>"
                         : "")
                textFormat: Text.StyledText
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }
        }
    }

    SettingsCards {
        id: cards

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(40)
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
        onEscapedLeft: Sound.edge()

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
