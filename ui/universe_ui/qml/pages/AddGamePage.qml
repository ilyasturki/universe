import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var args: ({})
    readonly property var form: api.screens.add

    signal closeRequested()
    signal installRequested(string source, string section)
    signal message(string text)

    readonly property var hints: editor.open ? editor.hints
        : confirm.open ? confirm.hints
        : [ { glyph: "A", label: cards.currentRow ? cards.currentRow.action || "Select" : "Select", dim: !cards.currentRow || form.busy },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)

    onArgsChanged: if (args.add) form.load()

    function activate(index, row) {
        if (form.busy) {
            Sound.edge();
        } else if (row.key === "pick_file") {
            Sound.panel();
            editor.edit({ type: "path", key: "pick_file", label: "Game file", value: "" }, function(path) {
                if (form.setFile(path))
                    pickRunner();
            });
        } else if (row.key === "store") {
            Sound.enter();
            page.installRequested(row.source, !row.available ? "modules" : row.loggedIn ? "install" : "login");
        } else if (row.key === "lutris") {
            importLutris();
        }
    }

    function pickRunner() {
        var choices = form.runnerChoices;
        if (choices.length === 0) {
            form.cancel();
            page.message("No runner set up: add one under Settings › Runners");
            Sound.edge();
            return;
        }
        editor.edit({ type: "enum", key: "runner", label: "Runner", value: choices[form.runnerIndex], choices: choices }, function(name) {
            form.pickRunner(choices.indexOf(name));
            editor.prompt("Title of the game", form.pendingTitle(), function(title) {
                form.addGame(title) !== "" ? Sound.enter() : Sound.edge();
            });
        });
    }

    function importLutris() {
        if (form.lutrisError !== "") {
            Sound.edge();
            page.message("Lutris was not found: " + form.lutrisError);
            return;
        }
        if (!form.lutris) {
            Sound.panel();
            form.previewLutris();
            return;
        }
        var n = form.lutris.imported.length;
        if (n === 0) {
            Sound.edge();
            page.message("Nothing new in Lutris");
            return;
        }
        confirm.ask({ message: "Import " + n + (n === 1 ? " game" : " games") + " from Lutris?",
                      detail: "Their hours and artwork come along. Games already in the library are left as they are.",
                      yes: "Import" },
                    function(yes) { if (yes) form.importLutris(); });
    }

    Connections {
        target: page.form
        function onMessage(text) { page.message(text); }
        // A preview that just came back answers the press that asked for it.
        function onLutrisChanged() {
            if (page.form.lutris && !page.form.busy && cards.currentRow && cards.currentRow.key === "lutris" && cards.activeFocus)
                page.importLutris();
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

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            CapsLabel {
                text: "LIBRARY"
            }

            Text {
                width: parent.width
                text: "Add a game"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: page.form.busy ? "Reading the Lutris library…" : "A file on this machine, a store, or what Lutris already has."
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
        columns: 1
        compact: true
        rows: page.form.rows
        groups: page.form.groups
        dimmed: editor.open || confirm.open

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

        onClosed: cards.forceActiveFocus()
    }

    ConfirmDialog {
        id: confirm
        anchors.fill: parent
        z: 4
        onClosed: cards.forceActiveFocus()
    }
}
