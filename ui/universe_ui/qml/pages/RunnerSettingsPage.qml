import QtQuick
import "../core"
import "../sound"
import "../ui"

// One runner's settings: its program and arguments, its options, a game added through it.
// Rows come built from the host; the type picks the control.
FocusScope {
    id: page

    focus: true

    property string runner: ""
    readonly property var form: api.screens.runner
    readonly property var info: form.info

    signal closeRequested()

    readonly property var hints: editor.open ? editor.hints
        : [ { glyph: "A", label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle"
                                : cards.currentRow && cards.currentRow.key === "add_file" ? "Pick a file" : "Change" },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property bool hasLogo: info.icon !== undefined && String(info.icon) !== "" && logo.status === Image.Ready

    onRunnerChanged: if (runner !== "") form.load(runner)

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else if (row.key === "add_file") {
            Sound.panel();
            editor.edit(index, { type: "path", key: "add_file", label: "Game file for " + info.name, value: "" });
        } else {
            Sound.panel();
            editor.edit(index, row);
        }
    }

    Connections {
        target: page.form
        function onMessage(text) { toast.show(text); }
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

        Image {
            id: logo
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            height: parent.height
            width: height
            source: page.info.icon ? Qt.resolvedUrl("../" + page.info.icon) : ""
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            sourceSize.height: 256
            smooth: true
            mipmap: true
            visible: page.hasLogo
        }

        Column {
            anchors.left: parent.left
            anchors.leftMargin: page.hasLogo ? logo.width + Theme.dp(28) : 0
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            CapsLabel {
                text: "RUNNER"
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

        onAccepted: function(index, value) {
            // callLater: the sheet's closed() follows accepted() and would close a prompt opened now.
            var add = page.form.row(index).key === "add_file";
            if (page.form.setValue(index, value) && add)
                Qt.callLater(function() { editor.prompt("add-title", "Title of the game", page.form.pendingTitle()); });
        }
        onPrompted: function(tag, value) {
            if (page.form.addGame(value) !== "")
                Sound.enter();
            else
                Sound.edge();
        }
        onClosed: cards.forceActiveFocus()
    }

    Toast {
        id: toast
        z: 4
    }
}
